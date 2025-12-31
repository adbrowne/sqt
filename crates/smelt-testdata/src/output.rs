//! Output formats for generated test data.

use crate::generator::{Event, Session, Visitor};
use arrow::array::{ArrayRef, Float64Array, StringArray, TimestampMillisecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use std::sync::Arc;

// ----------------------------------------------------------------------------
// SQL Output
// ----------------------------------------------------------------------------

/// SQL output format - generates INSERT statements.
pub struct SqlOutput {
    schema: String,
    batch_size: usize,
}

impl SqlOutput {
    /// Create a new SQL output formatter.
    pub fn new(schema: &str) -> Self {
        Self {
            schema: schema.to_string(),
            batch_size: 1000,
        }
    }

    /// Set the batch size for INSERT statements.
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Format visitors as SQL statements.
    pub fn format_visitors(&self, visitors: &[Visitor]) -> String {
        let mut sql = String::new();

        // Table creation
        sql.push_str(&format!(
            "CREATE TABLE IF NOT EXISTS {}.visitors (\n\
             \tvisitor_id VARCHAR PRIMARY KEY,\n\
             \tfirst_seen TIMESTAMP,\n\
             \tplatforms VARCHAR\n\
             );\n\n",
            self.schema
        ));

        if visitors.is_empty() {
            return sql;
        }

        // Batch inserts
        for chunk in visitors.chunks(self.batch_size) {
            sql.push_str(&format!(
                "INSERT INTO {}.visitors (visitor_id, first_seen, platforms) VALUES\n",
                self.schema
            ));

            let values: Vec<String> = chunk
                .iter()
                .map(|v| {
                    let platforms: Vec<&str> = v.platforms.iter().map(|p| p.as_str()).collect();
                    format!(
                        "('{}', '{}', '{}')",
                        v.visitor_id,
                        v.first_seen.format("%Y-%m-%d %H:%M:%S"),
                        platforms.join(",")
                    )
                })
                .collect();

            sql.push_str(&values.join(",\n"));
            sql.push_str(";\n\n");
        }

        sql
    }

    /// Format sessions as SQL statements.
    pub fn format_sessions(&self, sessions: &[Session]) -> String {
        let mut sql = String::new();

        sql.push_str(&format!(
            "CREATE TABLE IF NOT EXISTS {}.sessions (\n\
             \tsession_id VARCHAR PRIMARY KEY,\n\
             \tvisitor_id VARCHAR,\n\
             \tplatform VARCHAR,\n\
             \tstart_time TIMESTAMP,\n\
             \tduration_minutes DOUBLE\n\
             );\n\n",
            self.schema
        ));

        if sessions.is_empty() {
            return sql;
        }

        for chunk in sessions.chunks(self.batch_size) {
            sql.push_str(&format!(
                "INSERT INTO {}.sessions (session_id, visitor_id, platform, start_time, duration_minutes) VALUES\n",
                self.schema
            ));

            let values: Vec<String> = chunk
                .iter()
                .map(|s| {
                    format!(
                        "('{}', '{}', '{}', '{}', {:.2})",
                        s.session_id,
                        s.visitor_id,
                        s.platform.as_str(),
                        s.start_time.format("%Y-%m-%d %H:%M:%S"),
                        s.duration_minutes
                    )
                })
                .collect();

            sql.push_str(&values.join(",\n"));
            sql.push_str(";\n\n");
        }

        sql
    }

    /// Format events as SQL statements.
    pub fn format_events(&self, events: &[Event]) -> String {
        let mut sql = String::new();

        sql.push_str(&format!(
            "CREATE TABLE IF NOT EXISTS {}.events (\n\
             \tevent_id VARCHAR PRIMARY KEY,\n\
             \tsession_id VARCHAR,\n\
             \tvisitor_id VARCHAR,\n\
             \tevent_type VARCHAR,\n\
             \ttimestamp TIMESTAMP,\n\
             \tplatform VARCHAR\n\
             );\n\n",
            self.schema
        ));

        if events.is_empty() {
            return sql;
        }

        for chunk in events.chunks(self.batch_size) {
            sql.push_str(&format!(
                "INSERT INTO {}.events (event_id, session_id, visitor_id, event_type, timestamp, platform) VALUES\n",
                self.schema
            ));

            let values: Vec<String> = chunk
                .iter()
                .map(|e| {
                    format!(
                        "('{}', '{}', '{}', '{}', '{}', '{}')",
                        e.event_id,
                        e.session_id,
                        e.visitor_id,
                        e.event_type,
                        e.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                        e.platform.as_str()
                    )
                })
                .collect();

            sql.push_str(&values.join(",\n"));
            sql.push_str(";\n\n");
        }

        sql
    }

    /// Format all data as SQL statements.
    pub fn format_all(
        &self,
        visitors: &[Visitor],
        sessions: &[Session],
        events: &[Event],
    ) -> String {
        let mut sql = String::new();
        sql.push_str(&format!("CREATE SCHEMA IF NOT EXISTS {};\n\n", self.schema));
        sql.push_str(&self.format_visitors(visitors));
        sql.push_str(&self.format_sessions(sessions));
        sql.push_str(&self.format_events(events));
        sql
    }

    /// Format visitors as INSERT statements only (no CREATE TABLE).
    /// Used for streaming batch loading where tables are pre-created.
    pub fn format_visitors_data_only(&self, visitors: &[Visitor]) -> String {
        if visitors.is_empty() {
            return String::new();
        }

        let mut sql = String::new();
        for chunk in visitors.chunks(self.batch_size) {
            sql.push_str(&format!(
                "INSERT INTO {}.visitors (visitor_id, first_seen, platforms) VALUES\n",
                self.schema
            ));

            let values: Vec<String> = chunk
                .iter()
                .map(|v| {
                    let platforms: Vec<&str> = v.platforms.iter().map(|p| p.as_str()).collect();
                    format!(
                        "('{}', '{}', '{}')",
                        v.visitor_id,
                        v.first_seen.format("%Y-%m-%d %H:%M:%S"),
                        platforms.join(",")
                    )
                })
                .collect();

            sql.push_str(&values.join(",\n"));
            sql.push_str(";\n\n");
        }

        sql
    }

    /// Format sessions as INSERT statements only (no CREATE TABLE).
    pub fn format_sessions_data_only(&self, sessions: &[Session]) -> String {
        if sessions.is_empty() {
            return String::new();
        }

        let mut sql = String::new();
        for chunk in sessions.chunks(self.batch_size) {
            sql.push_str(&format!(
                "INSERT INTO {}.sessions (session_id, visitor_id, platform, start_time, duration_minutes) VALUES\n",
                self.schema
            ));

            let values: Vec<String> = chunk
                .iter()
                .map(|s| {
                    format!(
                        "('{}', '{}', '{}', '{}', {:.2})",
                        s.session_id,
                        s.visitor_id,
                        s.platform.as_str(),
                        s.start_time.format("%Y-%m-%d %H:%M:%S"),
                        s.duration_minutes
                    )
                })
                .collect();

            sql.push_str(&values.join(",\n"));
            sql.push_str(";\n\n");
        }

        sql
    }

    /// Format events as INSERT statements only (no CREATE TABLE).
    pub fn format_events_data_only(&self, events: &[Event]) -> String {
        if events.is_empty() {
            return String::new();
        }

        let mut sql = String::new();
        for chunk in events.chunks(self.batch_size) {
            sql.push_str(&format!(
                "INSERT INTO {}.events (event_id, session_id, visitor_id, event_type, timestamp, platform) VALUES\n",
                self.schema
            ));

            let values: Vec<String> = chunk
                .iter()
                .map(|e| {
                    format!(
                        "('{}', '{}', '{}', '{}', '{}', '{}')",
                        e.event_id,
                        e.session_id,
                        e.visitor_id,
                        e.event_type,
                        e.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                        e.platform.as_str()
                    )
                })
                .collect();

            sql.push_str(&values.join(",\n"));
            sql.push_str(";\n\n");
        }

        sql
    }
}

// ----------------------------------------------------------------------------
// Arrow Output
// ----------------------------------------------------------------------------

/// Arrow RecordBatch output format.
pub struct ArrowOutput;

impl ArrowOutput {
    /// Create a new Arrow output formatter.
    pub fn new() -> Self {
        Self
    }

    /// Convert visitors to an Arrow RecordBatch.
    pub fn visitors_to_batch(&self, visitors: &[Visitor]) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("visitor_id", DataType::Utf8, false),
            Field::new(
                "first_seen",
                DataType::Timestamp(TimeUnit::Millisecond, None),
                false,
            ),
            Field::new("platforms", DataType::Utf8, false),
        ]));

        let visitor_ids: StringArray = visitors
            .iter()
            .map(|v| Some(v.visitor_id.as_str()))
            .collect();

        let first_seen: TimestampMillisecondArray = visitors
            .iter()
            .map(|v| Some(v.first_seen.timestamp_millis()))
            .collect();

        let platforms: StringArray = visitors
            .iter()
            .map(|v| {
                let p: Vec<&str> = v.platforms.iter().map(|p| p.as_str()).collect();
                Some(p.join(","))
            })
            .collect();

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(visitor_ids) as ArrayRef,
                Arc::new(first_seen) as ArrayRef,
                Arc::new(platforms) as ArrayRef,
            ],
        )
        .expect("Failed to create RecordBatch")
    }

    /// Convert sessions to an Arrow RecordBatch.
    pub fn sessions_to_batch(&self, sessions: &[Session]) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("session_id", DataType::Utf8, false),
            Field::new("visitor_id", DataType::Utf8, false),
            Field::new("platform", DataType::Utf8, false),
            Field::new(
                "start_time",
                DataType::Timestamp(TimeUnit::Millisecond, None),
                false,
            ),
            Field::new("duration_minutes", DataType::Float64, false),
        ]));

        let session_ids: StringArray = sessions
            .iter()
            .map(|s| Some(s.session_id.as_str()))
            .collect();

        let visitor_ids: StringArray = sessions
            .iter()
            .map(|s| Some(s.visitor_id.as_str()))
            .collect();

        let platforms: StringArray = sessions.iter().map(|s| Some(s.platform.as_str())).collect();

        let start_times: TimestampMillisecondArray = sessions
            .iter()
            .map(|s| Some(s.start_time.timestamp_millis()))
            .collect();

        let durations: Float64Array = sessions.iter().map(|s| Some(s.duration_minutes)).collect();

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(session_ids) as ArrayRef,
                Arc::new(visitor_ids) as ArrayRef,
                Arc::new(platforms) as ArrayRef,
                Arc::new(start_times) as ArrayRef,
                Arc::new(durations) as ArrayRef,
            ],
        )
        .expect("Failed to create RecordBatch")
    }

    /// Convert events to an Arrow RecordBatch.
    pub fn events_to_batch(&self, events: &[Event]) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("event_id", DataType::Utf8, false),
            Field::new("session_id", DataType::Utf8, false),
            Field::new("visitor_id", DataType::Utf8, false),
            Field::new("event_type", DataType::Utf8, false),
            Field::new(
                "timestamp",
                DataType::Timestamp(TimeUnit::Millisecond, None),
                false,
            ),
            Field::new("platform", DataType::Utf8, false),
        ]));

        let event_ids: StringArray = events.iter().map(|e| Some(e.event_id.as_str())).collect();

        let session_ids: StringArray = events.iter().map(|e| Some(e.session_id.as_str())).collect();

        let visitor_ids: StringArray = events.iter().map(|e| Some(e.visitor_id.as_str())).collect();

        let event_types: StringArray = events.iter().map(|e| Some(e.event_type.as_str())).collect();

        let timestamps: TimestampMillisecondArray = events
            .iter()
            .map(|e| Some(e.timestamp.timestamp_millis()))
            .collect();

        let platforms: StringArray = events.iter().map(|e| Some(e.platform.as_str())).collect();

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(event_ids) as ArrayRef,
                Arc::new(session_ids) as ArrayRef,
                Arc::new(visitor_ids) as ArrayRef,
                Arc::new(event_types) as ArrayRef,
                Arc::new(timestamps) as ArrayRef,
                Arc::new(platforms) as ArrayRef,
            ],
        )
        .expect("Failed to create RecordBatch")
    }
}

impl Default for ArrowOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::TestDataBuilder;
    use crate::generator::TestDataGenerator;

    #[test]
    fn test_sql_output_visitors() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(5)
            .last_n_days(7)
            .build();

        let generator = TestDataGenerator::new(config);
        let visitors = generator.generate_visitors();

        let sql_output = SqlOutput::new("test_schema");
        let sql = sql_output.format_visitors(&visitors);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("test_schema.visitors"));
        assert!(sql.contains("INSERT INTO"));
        assert!(sql.contains("v_00000000")); // First visitor ID
    }

    #[test]
    fn test_sql_output_events() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(2)
            .last_n_days(7)
            .build();

        let generator = TestDataGenerator::new(config);
        let data = generator.generate();

        let sql_output = SqlOutput::new("test_schema");
        let sql = sql_output.format_events(&data.events);

        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("event_type VARCHAR"));
        assert!(sql.contains("INSERT INTO"));
    }

    #[test]
    fn test_arrow_output_visitors() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(10)
            .last_n_days(7)
            .build();

        let generator = TestDataGenerator::new(config);
        let visitors = generator.generate_visitors();

        let arrow_output = ArrowOutput::new();
        let batch = arrow_output.visitors_to_batch(&visitors);

        assert_eq!(batch.num_rows(), 10);
        assert_eq!(batch.num_columns(), 3);
    }

    #[test]
    fn test_arrow_output_sessions() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(5)
            .last_n_days(7)
            .build();

        let generator = TestDataGenerator::new(config);
        let visitors = generator.generate_visitors();
        let sessions = generator.generate_sessions(&visitors);

        let arrow_output = ArrowOutput::new();
        let batch = arrow_output.sessions_to_batch(&sessions);

        assert!(batch.num_rows() >= 5); // At least one session per visitor
        assert_eq!(batch.num_columns(), 5);
    }

    #[test]
    fn test_arrow_output_events() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(3)
            .last_n_days(7)
            .build();

        let generator = TestDataGenerator::new(config);
        let data = generator.generate();

        let arrow_output = ArrowOutput::new();
        let batch = arrow_output.events_to_batch(&data.events);

        assert!(batch.num_rows() > 0);
        assert_eq!(batch.num_columns(), 6);
    }
}
