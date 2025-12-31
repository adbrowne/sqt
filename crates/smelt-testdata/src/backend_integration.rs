//! Backend integration for loading test data.

use crate::generator::{GeneratedBatch, GeneratedData};
use crate::output::SqlOutput;
use async_trait::async_trait;
use smelt_backend::{Backend, BackendError};

/// Extension trait for loading test data into a backend.
#[async_trait]
pub trait TestDataLoader: Backend {
    /// Load generated test data into the backend.
    ///
    /// This will:
    /// 1. Ensure the schema exists
    /// 2. Create visitors, sessions, and events tables
    /// 3. Insert all generated data
    ///
    /// # Example
    /// ```ignore
    /// use smelt_testdata::{TestDataBuilder, TestDataGenerator, TestDataLoader};
    /// use smelt_backend_duckdb::DuckDbBackend;
    ///
    /// let backend = DuckDbBackend::new("test.db", "main").await?;
    /// let config = TestDataBuilder::new().visitors(100).build();
    /// let data = TestDataGenerator::new(config).generate();
    ///
    /// backend.load_test_data("testdata", &data).await?;
    /// ```
    async fn load_test_data(
        &self,
        schema: &str,
        data: &GeneratedData,
    ) -> Result<TestDataLoadResult, BackendError>;

    /// Load only visitors into the backend.
    async fn load_visitors(
        &self,
        schema: &str,
        visitors: &[crate::generator::Visitor],
    ) -> Result<usize, BackendError>;

    /// Load only sessions into the backend.
    async fn load_sessions(
        &self,
        schema: &str,
        sessions: &[crate::generator::Session],
    ) -> Result<usize, BackendError>;

    /// Load only events into the backend.
    async fn load_events(
        &self,
        schema: &str,
        events: &[crate::generator::Event],
    ) -> Result<usize, BackendError>;

    /// Load a single batch of generated data.
    ///
    /// Use this with `stream_batches()` for streaming large datasets:
    /// ```ignore
    /// // Create tables first
    /// backend.create_test_tables(schema).await?;
    ///
    /// // Stream batches
    /// for batch in generator.stream_batches(10_000) {
    ///     backend.load_batch(schema, &batch).await?;
    /// }
    /// ```
    async fn load_batch(
        &self,
        schema: &str,
        batch: &GeneratedBatch,
    ) -> Result<BatchLoadResult, BackendError>;

    /// Create the test data tables (visitors, sessions, events) without loading data.
    ///
    /// Call this once before streaming batches.
    async fn create_test_tables(&self, schema: &str) -> Result<(), BackendError>;
}

/// Result of loading test data.
#[derive(Debug, Clone)]
pub struct TestDataLoadResult {
    /// Number of visitors loaded
    pub visitors_loaded: usize,
    /// Number of sessions loaded
    pub sessions_loaded: usize,
    /// Number of events loaded
    pub events_loaded: usize,
}

/// Result of loading a single batch.
#[derive(Debug, Clone)]
pub struct BatchLoadResult {
    /// Batch index that was loaded
    pub batch_index: usize,
    /// Total batches expected
    pub total_batches: usize,
    /// Number of visitors loaded in this batch
    pub visitors_loaded: usize,
    /// Number of sessions loaded in this batch
    pub sessions_loaded: usize,
    /// Number of events loaded in this batch
    pub events_loaded: usize,
}

impl BatchLoadResult {
    /// Get the total number of rows loaded in this batch.
    pub fn total_rows(&self) -> usize {
        self.visitors_loaded + self.sessions_loaded + self.events_loaded
    }
}

impl std::fmt::Display for BatchLoadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Batch {}/{}: {} visitors, {} sessions, {} events",
            self.batch_index + 1,
            self.total_batches,
            self.visitors_loaded,
            self.sessions_loaded,
            self.events_loaded
        )
    }
}

impl TestDataLoadResult {
    /// Get the total number of rows loaded.
    pub fn total_rows(&self) -> usize {
        self.visitors_loaded + self.sessions_loaded + self.events_loaded
    }
}

impl std::fmt::Display for TestDataLoadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Loaded {} visitors, {} sessions, {} events ({} total rows)",
            self.visitors_loaded,
            self.sessions_loaded,
            self.events_loaded,
            self.total_rows()
        )
    }
}

#[async_trait]
impl<B: Backend + ?Sized> TestDataLoader for B {
    async fn load_test_data(
        &self,
        schema: &str,
        data: &GeneratedData,
    ) -> Result<TestDataLoadResult, BackendError> {
        // Ensure schema exists
        self.ensure_schema(schema).await?;

        // Load each entity type
        let visitors_loaded = self.load_visitors(schema, &data.visitors).await?;
        let sessions_loaded = self.load_sessions(schema, &data.sessions).await?;
        let events_loaded = self.load_events(schema, &data.events).await?;

        Ok(TestDataLoadResult {
            visitors_loaded,
            sessions_loaded,
            events_loaded,
        })
    }

    async fn load_visitors(
        &self,
        schema: &str,
        visitors: &[crate::generator::Visitor],
    ) -> Result<usize, BackendError> {
        let sql_output = SqlOutput::new(schema);
        let sql = sql_output.format_visitors(visitors);

        // Execute each statement
        for statement in sql.split(";\n").filter(|s| !s.trim().is_empty()) {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                self.execute_sql(&format!("{};", stmt)).await?;
            }
        }

        Ok(visitors.len())
    }

    async fn load_sessions(
        &self,
        schema: &str,
        sessions: &[crate::generator::Session],
    ) -> Result<usize, BackendError> {
        let sql_output = SqlOutput::new(schema);
        let sql = sql_output.format_sessions(sessions);

        for statement in sql.split(";\n").filter(|s| !s.trim().is_empty()) {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                self.execute_sql(&format!("{};", stmt)).await?;
            }
        }

        Ok(sessions.len())
    }

    async fn load_events(
        &self,
        schema: &str,
        events: &[crate::generator::Event],
    ) -> Result<usize, BackendError> {
        let sql_output = SqlOutput::new(schema);
        let sql = sql_output.format_events(events);

        for statement in sql.split(";\n").filter(|s| !s.trim().is_empty()) {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                self.execute_sql(&format!("{};", stmt)).await?;
            }
        }

        Ok(events.len())
    }

    async fn create_test_tables(&self, schema: &str) -> Result<(), BackendError> {
        // Ensure schema exists
        self.ensure_schema(schema).await?;

        // Create tables (same DDL as SqlOutput but just the CREATE statements)
        self.execute_sql(&format!(
            "CREATE TABLE IF NOT EXISTS {}.visitors (
                visitor_id VARCHAR PRIMARY KEY,
                first_seen TIMESTAMP,
                platforms VARCHAR
            );",
            schema
        ))
        .await?;

        self.execute_sql(&format!(
            "CREATE TABLE IF NOT EXISTS {}.sessions (
                session_id VARCHAR PRIMARY KEY,
                visitor_id VARCHAR,
                platform VARCHAR,
                start_time TIMESTAMP,
                duration_minutes DOUBLE
            );",
            schema
        ))
        .await?;

        self.execute_sql(&format!(
            "CREATE TABLE IF NOT EXISTS {}.events (
                event_id VARCHAR PRIMARY KEY,
                session_id VARCHAR,
                visitor_id VARCHAR,
                event_type VARCHAR,
                timestamp TIMESTAMP,
                platform VARCHAR
            );",
            schema
        ))
        .await?;

        Ok(())
    }

    async fn load_batch(
        &self,
        schema: &str,
        batch: &GeneratedBatch,
    ) -> Result<BatchLoadResult, BackendError> {
        let sql_output = SqlOutput::new(schema);

        // Load visitors (data only, tables already created)
        let visitors_loaded = if !batch.visitors.is_empty() {
            let sql = sql_output.format_visitors_data_only(&batch.visitors);
            for statement in sql.split(";\n").filter(|s| !s.trim().is_empty()) {
                let stmt = statement.trim();
                if !stmt.is_empty() {
                    self.execute_sql(&format!("{};", stmt)).await?;
                }
            }
            batch.visitors.len()
        } else {
            0
        };

        // Load sessions (data only)
        let sessions_loaded = if !batch.sessions.is_empty() {
            let sql = sql_output.format_sessions_data_only(&batch.sessions);
            for statement in sql.split(";\n").filter(|s| !s.trim().is_empty()) {
                let stmt = statement.trim();
                if !stmt.is_empty() {
                    self.execute_sql(&format!("{};", stmt)).await?;
                }
            }
            batch.sessions.len()
        } else {
            0
        };

        // Load events (data only)
        let events_loaded = if !batch.events.is_empty() {
            let sql = sql_output.format_events_data_only(&batch.events);
            for statement in sql.split(";\n").filter(|s| !s.trim().is_empty()) {
                let stmt = statement.trim();
                if !stmt.is_empty() {
                    self.execute_sql(&format!("{};", stmt)).await?;
                }
            }
            batch.events.len()
        } else {
            0
        };

        Ok(BatchLoadResult {
            batch_index: batch.batch_index,
            total_batches: batch.total_batches,
            visitors_loaded,
            sessions_loaded,
            events_loaded,
        })
    }
}
