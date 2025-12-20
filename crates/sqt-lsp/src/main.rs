/// sqt Language Server
///
/// Provides LSP features for sqt model files:
/// - Diagnostics (errors, warnings)
/// - Go-to-definition for ref() calls
/// - Completions (future)
/// - Hover information (future)

use std::path::PathBuf;
use std::sync::Arc;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tokio::sync::Mutex;

use sqt_db::{Database, Diagnostic as DbDiagnostic, DiagnosticSeverity as DbSeverity, Inputs, Semantic, Syntax};
use sqt_parser::ast::File as AstFile;

struct Backend {
    client: Client,
    db: Arc<Mutex<Database>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            db: Arc::new(Mutex::new(Database::default())),
        }
    }

    /// Convert our database diagnostic to LSP diagnostic
    fn to_lsp_diagnostic(&self, diag: &DbDiagnostic) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: Range {
                start: Position {
                    line: diag.range.start.line,
                    character: diag.range.start.column,
                },
                end: Position {
                    line: diag.range.end.line,
                    character: diag.range.end.column,
                },
            },
            severity: Some(match diag.severity {
                DbSeverity::Error => DiagnosticSeverity::ERROR,
                DbSeverity::Warning => DiagnosticSeverity::WARNING,
                DbSeverity::Info => DiagnosticSeverity::INFORMATION,
            }),
            message: diag.message.clone(),
            source: Some("sqt".to_string()),
            ..Default::default()
        }
    }

    /// Publish diagnostics for a file
    async fn publish_diagnostics(&self, uri: Url) {
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        let db = self.db.lock().await;
        let diagnostics = db.file_diagnostics(path);

        let lsp_diagnostics: Vec<lsp_types::Diagnostic> = diagnostics
            .iter()
            .map(|d| self.to_lsp_diagnostic(d))
            .collect();

        self.client
            .publish_diagnostics(uri, lsp_diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Get workspace folders if provided
        if let Some(workspace_folders) = params.workspace_folders {
            let mut db = self.db.lock().await;

            // Scan for all .sql files in workspace
            for folder in workspace_folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    if let Ok(entries) = std::fs::read_dir(path.join("models")) {
                        let mut files = Vec::new();

                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path.extension().and_then(|s| s.to_str()) == Some("sql") {
                                // Read file content and set it in the database
                                if let Ok(content) = std::fs::read_to_string(&entry_path) {
                                    db.set_file_text(entry_path.clone(), Arc::new(content));
                                    files.push(entry_path);
                                }
                            }
                        }

                        db.set_all_files(Arc::new(files));
                    }
                }
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "sqt language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        // Update file content in database
        let mut db = self.db.lock().await;
        db.set_file_text(path, Arc::new(params.text_document.text));
        drop(db);

        // Publish diagnostics
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        // Get new text (we use FULL sync, so there's only one change)
        if let Some(change) = params.content_changes.into_iter().next() {
            // Update in database - Salsa will handle incremental recomputation
            let mut db = self.db.lock().await;
            db.set_file_text(path, Arc::new(change.text));
            drop(db);

            // Publish diagnostics
            self.publish_diagnostics(uri).await;
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let db = self.db.lock().await;

        // Get file content and parse tree
        let text = db.file_text(path.clone());
        let parse = db.parse_file(path.clone());
        let syntax = parse.syntax();

        // Convert cursor position to offset
        let cursor_offset = {
            let mut offset = 0usize;
            let mut line = 0u32;
            let mut col = 0u32;

            for ch in text.chars() {
                if line == position.line && col == position.character {
                    break;
                }
                if ch == '\n' {
                    line += 1;
                    col = 0;
                } else {
                    col += 1;
                }
                offset += ch.len_utf8();
            }
            offset
        };

        // Find RefCall at cursor position using AST
        if let Some(file) = AstFile::cast(syntax) {
            for ref_call in file.refs() {
                let range = ref_call.range();
                let start: usize = range.start().into();
                let end: usize = range.end().into();

                // Check if cursor is within this ref call
                if cursor_offset >= start && cursor_offset <= end {
                    if let Some(ref_name) = ref_call.model_name() {
                        // Resolve the ref
                        if let Some(target_path) = db.resolve_ref(ref_name) {
                            if let Ok(target_uri) = Url::from_file_path(&target_path) {
                                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: target_uri,
                                    range: Range {
                                        start: Position::new(0, 0),
                                        end: Position::new(0, 0),
                                    },
                                })));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
