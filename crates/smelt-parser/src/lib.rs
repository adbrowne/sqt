/// smelt-parser - Rowan-based parser for smelt SQL files
///
/// This crate provides a standalone parser for smelt model files, which are
/// SQL files with template expressions like {{ ref('model_name') }}.
///
/// The parser is built on Rowan, providing:
/// - Lossless concrete syntax tree (CST)
/// - Error recovery (parse incomplete/invalid code)
/// - Position tracking for diagnostics and IDE features
///
/// This crate is standalone and can be used independently of the LSP or Salsa.
pub mod syntax_kind;
pub mod lexer;
pub mod parser;
pub mod ast;
pub mod printer;

pub use syntax_kind::SyntaxKind;
pub use parser::{parse, Parse, ParseError};
pub use ast::*;
pub use printer::{FormatMode, FormatContext};

/// Re-export Rowan types for convenience
pub use rowan::TextRange;
