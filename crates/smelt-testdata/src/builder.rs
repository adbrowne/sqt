//! Fluent builder for TestDataConfig.

use crate::config::{EventConfig, SchemaConfig, TestDataConfig, TimeRange};
use crate::distributions::{EventCountModel, PlatformAffinityModel, VisitorFrequencyModel};
use chrono::{DateTime, Utc};

/// Fluent builder for TestDataConfig.
///
/// # Example
/// ```
/// use smelt_testdata::TestDataBuilder;
///
/// let config = TestDataBuilder::new()
///     .seed(42)
///     .visitors(1000)
///     .last_n_days(30)
///     .ecommerce_events()
///     .build();
/// ```
pub struct TestDataBuilder {
    config: TestDataConfig,
}

impl TestDataBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            config: TestDataConfig::default(),
        }
    }

    /// Set the master seed for reproducibility.
    pub fn seed(mut self, seed: u64) -> Self {
        self.config.seed = seed;
        self
    }

    /// Set the time range for data generation.
    pub fn time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.config.time_range = TimeRange::new(start, end);
        self
    }

    /// Set number of days to generate (ending now).
    pub fn last_n_days(mut self, days: i64) -> Self {
        self.config.time_range = TimeRange::last_n_days(days);
        self
    }

    /// Set the time range to a specific month.
    pub fn month(mut self, year: i32, month: u32) -> Self {
        self.config.time_range = TimeRange::month(year, month);
        self
    }

    /// Set the number of unique visitors.
    pub fn visitors(mut self, count: usize) -> Self {
        self.config.visitors.count = count;
        self
    }

    /// Configure visitor frequency patterns.
    pub fn visitor_frequency(mut self, model: VisitorFrequencyModel) -> Self {
        self.config.visitors.frequency_model = model;
        self
    }

    /// Use high engagement visitor frequency.
    pub fn high_engagement_visitors(self) -> Self {
        self.visitor_frequency(VisitorFrequencyModel::high_engagement())
    }

    /// Use low engagement visitor frequency (high churn).
    pub fn low_engagement_visitors(self) -> Self {
        self.visitor_frequency(VisitorFrequencyModel::low_engagement())
    }

    /// Configure platform affinity patterns.
    pub fn platform_model(mut self, model: PlatformAffinityModel) -> Self {
        self.config.visitors.platform_model = model;
        self
    }

    /// Use mobile-first platform distribution.
    pub fn mobile_first(self) -> Self {
        self.platform_model(PlatformAffinityModel::mobile_first())
    }

    /// Use web-first platform distribution.
    pub fn web_first(self) -> Self {
        self.platform_model(PlatformAffinityModel::web_first())
    }

    /// Set custom event types with weights.
    pub fn event_types(mut self, types: Vec<(String, f64)>) -> Self {
        self.config.events.event_types = types;
        self
    }

    /// Use the full event configuration.
    pub fn event_config(mut self, config: EventConfig) -> Self {
        self.config.events = config;
        self
    }

    /// Use e-commerce event types preset.
    pub fn ecommerce_events(mut self) -> Self {
        self.config.events = EventConfig::ecommerce();
        self
    }

    /// Use SaaS/app event types preset.
    pub fn saas_events(mut self) -> Self {
        self.config.events = EventConfig::saas();
        self
    }

    /// Use media/content event types preset.
    pub fn media_events(mut self) -> Self {
        self.config.events = EventConfig::media();
        self
    }

    /// Set the events per session model.
    pub fn events_per_session(mut self, model: EventCountModel) -> Self {
        self.config.events.events_per_session = model;
        self
    }

    /// Set session duration range in minutes.
    pub fn session_duration(mut self, min_minutes: f64, max_minutes: f64) -> Self {
        self.config.events.session_duration_minutes = (min_minutes, max_minutes);
        self
    }

    /// Set the database schema name.
    pub fn db_schema(mut self, schema: &str) -> Self {
        self.config.schema.db_schema = schema.to_string();
        self
    }

    /// Set custom table names.
    pub fn table_names(mut self, visitors: &str, events: &str, sessions: Option<&str>) -> Self {
        self.config.schema.visitors_table = visitors.to_string();
        self.config.schema.events_table = events.to_string();
        self.config.schema.sessions_table = sessions.map(|s| s.to_string());
        self
    }

    /// Set the full schema configuration.
    pub fn schema_config(mut self, config: SchemaConfig) -> Self {
        self.config.schema = config;
        self
    }

    /// Build the configuration.
    pub fn build(self) -> TestDataConfig {
        self.config
    }
}

impl Default for TestDataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let config = TestDataBuilder::new().build();
        assert_eq!(config.seed, 42);
        assert_eq!(config.visitors.count, 1000);
    }

    #[test]
    fn test_builder_seed() {
        let config = TestDataBuilder::new().seed(123).build();
        assert_eq!(config.seed, 123);
    }

    #[test]
    fn test_builder_visitors() {
        let config = TestDataBuilder::new().visitors(500).build();
        assert_eq!(config.visitors.count, 500);
    }

    #[test]
    fn test_builder_ecommerce() {
        let config = TestDataBuilder::new().ecommerce_events().build();
        let has_product_view = config
            .events
            .event_types
            .iter()
            .any(|(name, _)| name == "product_view");
        assert!(has_product_view);
    }

    #[test]
    fn test_builder_chaining() {
        let config = TestDataBuilder::new()
            .seed(99)
            .visitors(2000)
            .last_n_days(60)
            .ecommerce_events()
            .mobile_first()
            .db_schema("myschema")
            .build();

        assert_eq!(config.seed, 99);
        assert_eq!(config.visitors.count, 2000);
        assert_eq!(config.schema.db_schema, "myschema");
    }
}
