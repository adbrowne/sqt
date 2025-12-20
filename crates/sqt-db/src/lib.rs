/// Salsa database for incremental compilation
///
/// This module defines the Salsa queries that power the LSP and optimizer.
/// Salsa automatically handles incremental recomputation when inputs change.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sqt_parser::{self, File as AstFile};

/// Input queries - these are set by the LSP when files change
#[salsa::query_group(InputsStorage)]
pub trait Inputs {
    /// Get the text content of a file
    /// This is an input query - set by LSP when file changes
    #[salsa::input]
    fn file_text(&self, path: PathBuf) -> Arc<String>;

    /// Get all file paths in the project
    #[salsa::input]
    fn all_files(&self) -> Arc<Vec<PathBuf>>;
}

/// Syntax queries - parsing and CST construction
#[salsa::query_group(SyntaxStorage)]
pub trait Syntax: Inputs {
    /// Parse a file into a CST
    fn parse_file(&self, path: PathBuf) -> Arc<sqt_parser::Parse>;

    /// Parse a file and extract model definitions
    /// Returns None if file doesn't contain a valid model
    fn parse_model(&self, path: PathBuf) -> Option<Arc<Model>>;

    /// Extract all ref() calls from a model with their positions
    fn model_refs(&self, path: PathBuf) -> Arc<Vec<RefLocation>>;

    /// Get all models in the project
    fn all_models(&self) -> Arc<HashMap<PathBuf, Model>>;
}

/// Semantic queries - name resolution, type checking, etc.
#[salsa::query_group(SemanticStorage)]
pub trait Semantic: Syntax {
    /// Resolve a ref() to the file path where it's defined
    /// Returns None if the ref is undefined
    fn resolve_ref(&self, model_name: String) -> Option<PathBuf>;

    /// Get all diagnostics for a file
    fn file_diagnostics(&self, path: PathBuf) -> Arc<Vec<Diagnostic>>;
}

/// The main database that combines all query groups
#[salsa::database(InputsStorage, SyntaxStorage, SemanticStorage)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for Database {}

// Query implementations

fn parse_file(db: &dyn Syntax, path: PathBuf) -> Arc<sqt_parser::Parse> {
    let text = db.file_text(path);
    Arc::new(sqt_parser::parse(&text))
}

fn parse_model(db: &dyn Syntax, path: PathBuf) -> Option<Arc<Model>> {
    // Extract model name from file path (e.g., models/users.sql -> users)
    let model_name = path
        .file_stem()?
        .to_str()?
        .to_string();

    // Parse file and check if it contains a valid SELECT statement
    let parse = db.parse_file(path.clone());
    let syntax = parse.syntax();
    let file = AstFile::cast(syntax)?;

    // Check if file has a SELECT statement
    if file.select_stmt().is_none() {
        return None;
    }

    Some(Arc::new(Model {
        name: model_name,
        path: path.clone(),
    }))
}

fn model_refs(db: &dyn Syntax, path: PathBuf) -> Arc<Vec<RefLocation>> {
    let parse = db.parse_file(path.clone());
    let text = db.file_text(path);
    let syntax = parse.syntax();

    // Use AST to extract all ref calls with positions
    if let Some(file) = AstFile::cast(syntax) {
        let refs: Vec<RefLocation> = file
            .refs()
            .filter_map(|ref_call| {
                let name = ref_call.model_name()?;
                let text_range = ref_call.name_range().unwrap_or(ref_call.range());
                let range = sqt_parser::ast::text_range_to_range(&text, text_range);

                Some(RefLocation { name, range })
            })
            .collect();

        Arc::new(refs)
    } else {
        Arc::new(Vec::new())
    }
}

fn all_models(db: &dyn Syntax) -> Arc<HashMap<PathBuf, Model>> {
    let files = db.all_files();
    let mut models = HashMap::new();

    for path in files.iter() {
        if let Some(model) = db.parse_model(path.clone()) {
            models.insert(path.clone(), (*model).clone());
        }
    }

    Arc::new(models)
}

fn resolve_ref(db: &dyn Semantic, model_name: String) -> Option<PathBuf> {
    let models = db.all_models();

    // Find the model with this name
    models.iter()
        .find(|(_, model)| model.name == model_name)
        .map(|(path, _)| path.clone())
}

fn file_diagnostics(db: &dyn Semantic, path: PathBuf) -> Arc<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // Add parse errors
    let parse = db.parse_file(path.clone());
    for error in parse.errors.iter() {
        let text = db.file_text(path.clone());
        let range = sqt_parser::ast::text_range_to_range(&text, error.range);

        diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Error,
            message: error.message.clone(),
            range,
        });
    }

    // Check if model is valid
    if db.parse_model(path.clone()).is_none() {
        // Only report error if file is supposed to be a model (in models/ directory)
        if path.to_str().map(|s| s.contains("models/")).unwrap_or(false) {
            diagnostics.push(Diagnostic {
                severity: DiagnosticSeverity::Warning,
                message: "File does not contain a valid SQL query".to_string(),
                range: Range {
                    start: Position { line: 0, column: 0 },
                    end: Position { line: 0, column: 0 },
                },
            });
        }
        return Arc::new(diagnostics);
    }

    // Check for undefined refs with accurate positions
    let refs = db.model_refs(path.clone());
    for ref_loc in refs.iter() {
        if db.resolve_ref(ref_loc.name.clone()).is_none() {
            diagnostics.push(Diagnostic {
                severity: DiagnosticSeverity::Error,
                message: format!("Undefined model reference: '{}'", ref_loc.name),
                range: ref_loc.range,
            });
        }
    }

    Arc::new(diagnostics)
}

/// Represents a model (SQL file in models/ directory)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub path: PathBuf,
}

/// Reference location with position information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefLocation {
    pub name: String,
    pub range: Range,
}

/// Position in a file (line, column)
pub type Position = sqt_parser::ast::Position;

/// Range in a file (start, end)
pub type Range = sqt_parser::ast::Range;

/// Represents a diagnostic (error, warning, info)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}
