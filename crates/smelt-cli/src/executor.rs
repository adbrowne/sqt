use crate::compiler::CompiledModel;
use crate::config::{Materialization, SourceConfig};
use crate::errors::CliError;
use anyhow::{Context, Result};
use arrow::array::RecordBatch;
use duckdb::Connection;
use std::path::Path;
use std::time::Duration;

pub struct DuckDbExecutor {
    connection: Connection,
    schema: String,
}

pub struct ExecutionResult {
    pub model_name: String,
    pub duration: Duration,
    pub row_count: usize,
    pub preview: Option<Vec<RecordBatch>>,
}

impl DuckDbExecutor {
    pub fn new(database_path: &Path, schema: &str) -> Result<Self> {
        // Create parent directory if needed
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // Open file-based connection (persistent)
        let connection = Connection::open(database_path)
            .with_context(|| format!("Failed to open DuckDB database: {:?}", database_path))?;

        // Ensure schema exists
        connection
            .execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema), [])
            .with_context(|| format!("Failed to create schema: {}", schema))?;

        Ok(Self {
            connection,
            schema: schema.to_string(),
        })
    }

    pub fn validate_sources(&self, sources: &SourceConfig) -> Result<()> {
        let mut missing = Vec::new();

        for (schema_name, schema) in &sources.sources {
            for table_name in schema.tables.keys() {
                // Check if table exists in DuckDB
                let query = "SELECT COUNT(*) > 0 FROM information_schema.tables WHERE table_schema = ? AND table_name = ?";

                let exists: bool = self
                    .connection
                    .query_row(query, [schema_name, table_name], |row| row.get(0))
                    .unwrap_or(false);

                if !exists {
                    missing.push(format!("{}.{}", schema_name, table_name));
                }
            }
        }

        if !missing.is_empty() {
            return Err(CliError::SourceTablesNotFound { missing }.into());
        }

        Ok(())
    }

    pub fn execute_model(
        &self,
        compiled: &CompiledModel,
        show_results: bool,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

        // Generate DDL statements
        let statements = self.generate_ddl(compiled);

        // Execute each statement
        for stmt in &statements {
            self.connection.execute(stmt, []).map_err(|e| {
                CliError::ExecutionError {
                    model: compiled.name.clone(),
                    sql: compiled.sql.clone(),
                    source: e.into(),
                }
            })?;
        }

        let duration = start.elapsed();

        // Get row count
        let row_count = self.get_row_count(&compiled.name)?;

        // Optionally show results
        let preview = if show_results {
            Some(self.query_preview(&compiled.name)?)
        } else {
            None
        };

        Ok(ExecutionResult {
            model_name: compiled.name.clone(),
            duration,
            row_count,
            preview,
        })
    }

    fn generate_ddl(&self, model: &CompiledModel) -> Vec<String> {
        let table_name = format!("{}.{}", self.schema, model.name);

        let mut statements = Vec::new();

        match model.materialization {
            Materialization::Table => {
                statements.push(format!("DROP TABLE IF EXISTS {}", table_name));
                statements.push(format!("CREATE TABLE {} AS {}", table_name, model.sql));
            }
            Materialization::View => {
                statements.push(format!("DROP VIEW IF EXISTS {}", table_name));
                statements.push(format!("CREATE VIEW {} AS {}", table_name, model.sql));
            }
        }

        statements
    }

    fn query_preview(&self, model_name: &str) -> Result<Vec<RecordBatch>> {
        let sql = format!("SELECT * FROM {}.{} LIMIT 10", self.schema, model_name);

        let mut stmt = self
            .connection
            .prepare(&sql)
            .with_context(|| format!("Failed to prepare preview query for {}", model_name))?;

        let arrow_result = stmt
            .query_arrow([])
            .with_context(|| format!("Failed to execute preview query for {}", model_name))?;

        Ok(arrow_result.collect())
    }

    fn get_row_count(&self, model_name: &str) -> Result<usize> {
        let sql = format!("SELECT COUNT(*) FROM {}.{}", self.schema, model_name);

        let count: usize = self
            .connection
            .query_row(&sql, [], |row| row.get(0))
            .with_context(|| format!("Failed to get row count for {}", model_name))?;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Materialization;
    use tempfile::TempDir;

    #[test]
    fn test_executor_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.duckdb");

        let _executor = DuckDbExecutor::new(&db_path, "main").unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn test_execute_table() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.duckdb");

        let executor = DuckDbExecutor::new(&db_path, "main").unwrap();

        let compiled = CompiledModel {
            name: "test_model".to_string(),
            sql: "SELECT 1 as id, 'test' as name".to_string(),
            materialization: Materialization::Table,
        };

        let result = executor.execute_model(&compiled, false).unwrap();

        assert_eq!(result.model_name, "test_model");
        assert_eq!(result.row_count, 1);
        assert!(result.preview.is_none());
    }

    #[test]
    fn test_execute_view() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.duckdb");

        let executor = DuckDbExecutor::new(&db_path, "main").unwrap();

        let compiled = CompiledModel {
            name: "test_view".to_string(),
            sql: "SELECT 1 as id, 'test' as name".to_string(),
            materialization: Materialization::View,
        };

        let result = executor.execute_model(&compiled, false).unwrap();

        assert_eq!(result.model_name, "test_view");
        assert_eq!(result.row_count, 1);
    }

    #[test]
    fn test_execute_with_preview() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.duckdb");

        let executor = DuckDbExecutor::new(&db_path, "main").unwrap();

        let compiled = CompiledModel {
            name: "test_preview".to_string(),
            sql: "SELECT 1 as id UNION SELECT 2 UNION SELECT 3".to_string(),
            materialization: Materialization::Table,
        };

        let result = executor.execute_model(&compiled, true).unwrap();

        assert_eq!(result.row_count, 3);
        assert!(result.preview.is_some());

        let batches = result.preview.unwrap();
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total_rows, 3);
    }
}
