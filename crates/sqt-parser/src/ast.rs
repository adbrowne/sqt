/// Typed AST wrappers over Rowan CST

use crate::syntax_kind::{SyntaxNode, SyntaxToken};
use crate::SyntaxKind::*;
use rowan::TextRange;

/// Root file node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct File(SyntaxNode);

impl File {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == FILE {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn select_stmt(&self) -> Option<SelectStmt> {
        self.0.children().find_map(SelectStmt::cast)
    }

    pub fn refs(&self) -> impl Iterator<Item = RefCall> + '_ {
        self.0.descendants().filter_map(RefCall::cast)
    }
}

/// SELECT statement
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectStmt(SyntaxNode);

impl SelectStmt {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == SELECT_STMT {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn select_list(&self) -> Option<SelectList> {
        self.0.children().find_map(SelectList::cast)
    }

    pub fn from_clause(&self) -> Option<FromClause> {
        self.0.children().find_map(FromClause::cast)
    }

    pub fn where_clause(&self) -> Option<WhereClause> {
        self.0.children().find_map(WhereClause::cast)
    }
}

/// SELECT list (columns)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectList(SyntaxNode);

impl SelectList {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == SELECT_LIST {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn items(&self) -> impl Iterator<Item = SelectItem> + '_ {
        self.0.children().filter_map(SelectItem::cast)
    }
}

/// SELECT item (column or expression with optional alias)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectItem(SyntaxNode);

impl SelectItem {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == SELECT_ITEM {
            Some(Self(node))
        } else {
            None
        }
    }
}

/// FROM clause
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FromClause(SyntaxNode);

impl FromClause {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == FROM_CLAUSE {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn table_refs(&self) -> impl Iterator<Item = TableRef> + '_ {
        self.0.children().filter_map(TableRef::cast)
    }
}

/// Table reference (identifier or template expression)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableRef(SyntaxNode);

impl TableRef {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == TABLE_REF {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn is_template(&self) -> bool {
        self.0.children().any(|n| n.kind() == TEMPLATE_EXPR)
    }

    pub fn template_expr(&self) -> Option<TemplateExpr> {
        self.0.children().find_map(TemplateExpr::cast)
    }

    pub fn identifier(&self) -> Option<String> {
        self.0
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == IDENT)
            .map(|t| t.text().to_string())
    }
}

/// WHERE clause
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhereClause(SyntaxNode);

impl WhereClause {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == WHERE_CLAUSE {
            Some(Self(node))
        } else {
            None
        }
    }
}

/// Template expression {{ ... }}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateExpr(SyntaxNode);

impl TemplateExpr {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == TEMPLATE_EXPR {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn ref_call(&self) -> Option<RefCall> {
        self.0.children().find_map(RefCall::cast)
    }
}

/// ref('model_name') call
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefCall(SyntaxNode);

impl RefCall {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == REF_CALL {
            Some(Self(node))
        } else {
            None
        }
    }

    /// Get the model name from the ref call
    pub fn model_name(&self) -> Option<String> {
        self.0
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == STRING)
            .map(|t| {
                let text = t.text();
                // Strip quotes
                text.trim_start_matches('\'')
                    .trim_start_matches('"')
                    .trim_end_matches('\'')
                    .trim_end_matches('"')
                    .to_string()
            })
    }

    /// Get the text range of the model name (for diagnostics)
    pub fn range(&self) -> TextRange {
        self.0.text_range()
    }

    /// Get the text range of just the model name string (inside quotes)
    pub fn name_range(&self) -> Option<TextRange> {
        self.0
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == STRING)
            .map(|t| t.text_range())
    }
}

/// Helper to convert TextRange offset to line/column position
pub fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut column = 0u32;

    for (i, ch) in text.chars().enumerate() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Position { line, column }
}

/// Helper to convert TextRange to LSP Range
pub fn text_range_to_range(text: &str, range: TextRange) -> Range {
    let start = offset_to_position(text, usize::from(range.start()));
    let end = offset_to_position(text, usize::from(range.end()));
    Range { start, end }
}

/// Position (line, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// Range (start, end positions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}
