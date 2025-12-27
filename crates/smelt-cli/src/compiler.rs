use crate::config::{Config, Materialization};
use crate::discovery::ModelFile;
use crate::errors::{extract_snippet, text_range_to_line_col, CliError};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct CompiledModel {
    pub name: String,
    pub sql: String,
    pub materialization: Materialization,
}

pub struct SqlCompiler {
    config: Config,
}

impl SqlCompiler {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Compile a model's SQL by replacing smelt.ref() calls with table references
    pub fn compile(&self, model: &ModelFile, schema: &str) -> Result<CompiledModel> {
        // ERROR if any named parameters detected
        for ref_info in &model.refs {
            if ref_info.has_named_params {
                let (line, col) = text_range_to_line_col(&model.content, ref_info.range);
                let snippet = extract_snippet(&model.content, ref_info.range, 0);

                return Err(CliError::NamedParametersNotSupported {
                    model: model.name.clone(),
                    file: model.path.clone(),
                    line,
                    col,
                    snippet,
                }
                .into());
            }
        }

        // Replace refs - we'll do simple string replacement for now
        // For a production implementation, we'd want AST-based rewriting
        let mut compiled_sql = model.content.clone();

        // Collect all unique refs for replacement
        let unique_refs: std::collections::HashSet<_> =
            model.refs.iter().map(|r| r.model_name.as_str()).collect();

        // Replace each ref pattern
        for ref_name in unique_refs {
            let pattern = format!("smelt.ref('{}')", ref_name);
            let replacement = format!("{}.{}", schema, ref_name);
            compiled_sql = compiled_sql.replace(&pattern, &replacement);
        }

        Ok(CompiledModel {
            name: model.name.clone(),
            sql: compiled_sql,
            materialization: self.config.get_materialization(&model.name),
        })
    }

    /// Compile a model with custom SQL (e.g., for transformed queries).
    /// This is used for incremental processing where the SQL has been transformed.
    pub fn compile_with_sql(
        &self,
        model: &ModelFile,
        schema: &str,
        sql: &str,
    ) -> Result<CompiledModel> {
        // Replace refs in the provided SQL
        let unique_refs: std::collections::HashSet<_> =
            model.refs.iter().map(|r| r.model_name.as_str()).collect();

        let mut compiled_sql = sql.to_string();
        for ref_name in unique_refs {
            let pattern = format!("smelt.ref('{}')", ref_name);
            let replacement = format!("{}.{}", schema, ref_name);
            compiled_sql = compiled_sql.replace(&pattern, &replacement);
        }

        Ok(CompiledModel {
            name: model.name.clone(),
            sql: compiled_sql,
            materialization: self.config.get_materialization(&model.name),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ModelConfig, Target};
    use std::collections::HashMap;

    fn make_test_config() -> Config {
        let mut targets = HashMap::new();
        targets.insert(
            "dev".to_string(),
            Target {
                target_type: "duckdb".to_string(),
                database: Some("test.duckdb".to_string()),
                schema: "main".to_string(),
                connect_url: None,
                catalog: None,
            },
        );

        Config {
            name: "test".to_string(),
            version: 1,
            model_paths: vec!["models".to_string()],
            targets,
            default_materialization: Materialization::View,
            models: HashMap::new(),
        }
    }

    #[test]
    fn test_simple_ref_replacement() {
        use crate::discovery::RefInfo;
        use rowan::TextRange;

        let sql = r#"
SELECT
    user_id,
    COUNT(*) as session_count
FROM smelt.ref('raw_events')
GROUP BY user_id
"#;

        let model = ModelFile {
            name: "user_stats".to_string(),
            path: "models/user_stats.sql".into(),
            content: sql.to_string(),
            refs: vec![RefInfo {
                model_name: "raw_events".to_string(),
                has_named_params: false,
                range: TextRange::default(),
            }],
            parse_errors: Vec::new(),
        };

        let config = make_test_config();
        let compiler = SqlCompiler::new(config);

        let compiled = compiler.compile(&model, "main").unwrap();

        assert!(compiled.sql.contains("FROM main.raw_events"));
        assert!(!compiled.sql.contains("smelt.ref"));
    }

    #[test]
    fn test_multiple_refs() {
        use crate::discovery::RefInfo;
        use rowan::TextRange;

        let sql = r#"
SELECT a.user_id, b.session_id
FROM smelt.ref('model_a') a
JOIN smelt.ref('model_b') b ON a.id = b.id
"#;

        let model = ModelFile {
            name: "combined".to_string(),
            path: "models/combined.sql".into(),
            content: sql.to_string(),
            refs: vec![
                RefInfo {
                    model_name: "model_a".to_string(),
                    has_named_params: false,
                    range: TextRange::default(),
                },
                RefInfo {
                    model_name: "model_b".to_string(),
                    has_named_params: false,
                    range: TextRange::default(),
                },
            ],
            parse_errors: Vec::new(),
        };

        let config = make_test_config();
        let compiler = SqlCompiler::new(config);

        let compiled = compiler.compile(&model, "main").unwrap();

        assert!(compiled.sql.contains("FROM main.model_a a"));
        assert!(compiled.sql.contains("JOIN main.model_b b"));
        assert!(!compiled.sql.contains("smelt.ref"));
    }

    #[test]
    fn test_named_params_error() {
        use crate::discovery::RefInfo;
        use rowan::TextRange;

        let sql = r#"
SELECT user_id
FROM smelt.ref('raw_events', filter => event_type = 'page_view')
"#;

        let model = ModelFile {
            name: "filtered".to_string(),
            path: "models/filtered.sql".into(),
            content: sql.to_string(),
            refs: vec![RefInfo {
                model_name: "raw_events".to_string(),
                has_named_params: true,
                range: TextRange::new(0u32.into(), 10u32.into()),
            }],
            parse_errors: Vec::new(),
        };

        let config = make_test_config();
        let compiler = SqlCompiler::new(config);

        let result = compiler.compile(&model, "main");
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("named parameters"));
        assert!(err_msg.contains("not yet supported"));
    }

    #[test]
    fn test_materialization_from_config() {
        let model = ModelFile {
            name: "test_model".to_string(),
            path: "models/test_model.sql".into(),
            content: "SELECT 1".to_string(),
            refs: vec![],
            parse_errors: Vec::new(),
        };

        let mut config = make_test_config();
        config.models.insert(
            "test_model".to_string(),
            ModelConfig {
                materialization: Some(Materialization::Table),
                incremental: None,
            },
        );

        let compiler = SqlCompiler::new(config);
        let compiled = compiler.compile(&model, "main").unwrap();

        assert!(matches!(
            compiled.materialization,
            Materialization::Table
        ));
    }
}
