//! Common types used across backends.

use arrow::array::RecordBatch;
use std::time::Duration;

/// Result of executing a model.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Name of the model that was executed.
    pub model_name: String,

    /// How long execution took.
    pub duration: Duration,

    /// Number of rows in the resulting table/view.
    pub row_count: usize,

    /// Optional preview of the first few rows.
    pub preview: Option<Vec<RecordBatch>>,
}

/// How a model should be materialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Materialization {
    /// Materialize as a table (persisted).
    Table,

    /// Materialize as a view (computed on query).
    #[default]
    View,
}

impl std::fmt::Display for Materialization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Materialization::Table => write!(f, "table"),
            Materialization::View => write!(f, "view"),
        }
    }
}

impl std::str::FromStr for Materialization {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Materialization::Table),
            "view" => Ok(Materialization::View),
            _ => Err(format!("Unknown materialization: {}", s)),
        }
    }
}
