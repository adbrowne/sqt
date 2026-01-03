//! Deterministic data generation for smelt.
//!
//! This crate provides proptest-inspired composable generators for creating
//! test data with deterministic output based on a seed value.

pub mod gen;
pub mod generators;
pub mod parquet;
pub mod session;

pub use gen::Gen;
pub use generators::*;
pub use session::{
    generate_day_seeds, DayGenerator, Session, SessionGenerator, Visitor, VisitorPool,
};
