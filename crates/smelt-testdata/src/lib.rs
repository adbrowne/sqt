//! Test data generation for smelt - realistic behavioral event data.
//!
//! This crate provides tools for generating deterministic, realistic test data
//! for behavioral analytics scenarios. It's inspired by property-based testing
//! libraries like QuickCheck and Hedgehog, but focused on volume generation
//! rather than property verification.
//!
//! # Key Features
//!
//! - **Deterministic**: Same seed always produces identical output
//! - **Realistic distributions**: Power law for visitor frequency, platform affinity
//! - **Composable generators**: Build complex generators from simple ones
//! - **Multiple output formats**: SQL statements, Arrow RecordBatches
//! - **Backend integration**: Load directly into DuckDB or other backends
//!
//! # Quick Start
//!
//! ```rust
//! use smelt_testdata::{TestDataBuilder, TestDataGenerator, presets};
//!
//! // Using a preset
//! let config = presets::unit_test();
//! let generator = TestDataGenerator::new(config);
//! let data = generator.generate();
//!
//! println!("{}", data.summary());
//! ```
//!
//! # Custom Configuration
//!
//! ```rust
//! use smelt_testdata::{TestDataBuilder, TestDataGenerator};
//!
//! let config = TestDataBuilder::new()
//!     .seed(42)                    // Reproducible
//!     .visitors(1000)              // 1000 unique visitors
//!     .last_n_days(30)             // 30 days of data
//!     .ecommerce_events()          // E-commerce event types
//!     .mobile_first()              // Mobile-heavy platform distribution
//!     .build();
//!
//! let generator = TestDataGenerator::new(config);
//! let data = generator.generate();
//! ```
//!
//! # SQL Output
//!
//! ```rust
//! use smelt_testdata::{TestDataBuilder, TestDataGenerator, SqlOutput};
//!
//! let config = TestDataBuilder::new().visitors(100).build();
//! let data = TestDataGenerator::new(config).generate();
//!
//! let sql = SqlOutput::new("testdata");
//! println!("{}", sql.format_all(&data.visitors, &data.sessions, &data.events));
//! ```
//!
//! # Arrow Output
//!
//! ```rust
//! use smelt_testdata::{TestDataBuilder, TestDataGenerator, ArrowOutput};
//!
//! let config = TestDataBuilder::new().visitors(100).build();
//! let data = TestDataGenerator::new(config).generate();
//!
//! let arrow = ArrowOutput::new();
//! let events_batch = arrow.events_to_batch(&data.events);
//! println!("Generated {} event rows", events_batch.num_rows());
//! ```
//!
//! # Backend Integration
//!
//! ```ignore
//! use smelt_testdata::{TestDataBuilder, TestDataGenerator, TestDataLoader, presets};
//! use smelt_backend_duckdb::DuckDbBackend;
//!
//! let backend = DuckDbBackend::new("test.db", "main").await?;
//! let data = TestDataGenerator::new(presets::unit_test()).generate();
//!
//! let result = backend.load_test_data("testdata", &data).await?;
//! println!("{}", result);
//! ```

pub mod backend_integration;
pub mod builder;
pub mod config;
pub mod core;
pub mod distributions;
pub mod generator;
pub mod output;
pub mod presets;
pub mod rng;

// Re-export main types for convenience
pub use backend_integration::{BatchLoadResult, TestDataLoadResult, TestDataLoader};
pub use builder::TestDataBuilder;
pub use config::{EventConfig, SchemaConfig, TestDataConfig, TimeRange, VisitorConfig};
pub use core::{Generator, GeneratorExt};
pub use distributions::{
    EventCountModel, Platform, PlatformAffinityModel, PowerLaw, VisitorFrequencyModel,
};
pub use generator::{
    Event, GeneratedBatch, GeneratedData, Session, StreamingBatchIterator, TestDataGenerator,
    Visitor,
};
pub use output::{ArrowOutput, SqlOutput};
pub use rng::SeededRngFactory;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end() {
        let config = TestDataBuilder::new()
            .seed(12345)
            .visitors(50)
            .last_n_days(7)
            .ecommerce_events()
            .build();

        let generator = TestDataGenerator::new(config);
        let data = generator.generate();

        // Verify we have data
        assert_eq!(data.visitors.len(), 50);
        assert!(!data.sessions.is_empty());
        assert!(!data.events.is_empty());

        // Verify SQL output works
        let sql = SqlOutput::new("test");
        let visitors_sql = sql.format_visitors(&data.visitors);
        assert!(visitors_sql.contains("CREATE TABLE"));
        assert!(visitors_sql.contains("INSERT INTO"));

        // Verify Arrow output works
        let arrow = ArrowOutput::new();
        let batch = arrow.events_to_batch(&data.events);
        assert_eq!(batch.num_rows(), data.events.len());
    }

    #[test]
    fn test_determinism_across_calls() {
        let config1 = TestDataBuilder::new()
            .seed(999)
            .visitors(100)
            .last_n_days(14)
            .build();

        let config2 = TestDataBuilder::new()
            .seed(999)
            .visitors(100)
            .last_n_days(14)
            .build();

        let data1 = TestDataGenerator::new(config1).generate();
        let data2 = TestDataGenerator::new(config2).generate();

        // Should have identical counts
        assert_eq!(data1.visitors.len(), data2.visitors.len());
        assert_eq!(data1.sessions.len(), data2.sessions.len());
        assert_eq!(data1.events.len(), data2.events.len());

        // Should have identical IDs
        for (v1, v2) in data1.visitors.iter().zip(data2.visitors.iter()) {
            assert_eq!(v1.visitor_id, v2.visitor_id);
        }
    }

    #[test]
    fn test_preset_usage() {
        let config = presets::ecommerce();
        let data = TestDataGenerator::new(config).generate();

        // E-commerce should have purchase events
        let has_purchase = data.events.iter().any(|e| e.event_type == "purchase");
        assert!(has_purchase);
    }
}
