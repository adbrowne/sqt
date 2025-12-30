//! Configuration structures for test data generation.

use crate::distributions::{EventCountModel, PlatformAffinityModel, VisitorFrequencyModel};
use chrono::{DateTime, Duration, Utc};

/// Top-level configuration for test data generation.
#[derive(Clone)]
pub struct TestDataConfig {
    /// Master seed for reproducibility
    pub seed: u64,

    /// Time range for generated data
    pub time_range: TimeRange,

    /// Visitor generation settings
    pub visitors: VisitorConfig,

    /// Event generation settings
    pub events: EventConfig,

    /// Schema settings (table names)
    pub schema: SchemaConfig,
}

impl Default for TestDataConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            time_range: TimeRange::last_n_days(30),
            visitors: VisitorConfig::default(),
            events: EventConfig::default(),
            schema: SchemaConfig::default(),
        }
    }
}

/// Time range for generated data.
#[derive(Clone)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    /// Create a time range with explicit start and end.
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// Create a time range covering the last N days (ending now).
    pub fn last_n_days(days: i64) -> Self {
        let end = Utc::now();
        let start = end - Duration::days(days);
        Self { start, end }
    }

    /// Create a time range for a specific year and month.
    pub fn month(year: i32, month: u32) -> Self {
        use chrono::NaiveDate;
        let start = NaiveDate::from_ymd_opt(year, month, 1)
            .expect("Invalid date")
            .and_hms_opt(0, 0, 0)
            .expect("Invalid time")
            .and_utc();

        let next_month = if month == 12 { 1 } else { month + 1 };
        let next_year = if month == 12 { year + 1 } else { year };
        let end = NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .expect("Invalid date")
            .and_hms_opt(0, 0, 0)
            .expect("Invalid time")
            .and_utc()
            - Duration::seconds(1);

        Self { start, end }
    }

    /// Get the duration of this time range.
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }

    /// Get the number of days in this time range.
    pub fn num_days(&self) -> i64 {
        self.duration().num_days()
    }
}

impl Default for TimeRange {
    fn default() -> Self {
        Self::last_n_days(30)
    }
}

/// Visitor generation configuration.
#[derive(Clone)]
pub struct VisitorConfig {
    /// Number of unique visitors to generate
    pub count: usize,

    /// Visit frequency distribution
    pub frequency_model: VisitorFrequencyModel,

    /// Platform affinity distribution
    pub platform_model: PlatformAffinityModel,
}

impl Default for VisitorConfig {
    fn default() -> Self {
        Self {
            count: 1000,
            frequency_model: VisitorFrequencyModel::default(),
            platform_model: PlatformAffinityModel::default(),
        }
    }
}

/// Event generation configuration.
#[derive(Clone)]
pub struct EventConfig {
    /// Event types and their relative weights
    pub event_types: Vec<(String, f64)>,

    /// Events per session distribution
    pub events_per_session: EventCountModel,

    /// Session duration range in minutes (min, max)
    pub session_duration_minutes: (f64, f64),
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            event_types: vec![
                ("page_view".to_string(), 0.50),
                ("click".to_string(), 0.25),
                ("scroll".to_string(), 0.10),
                ("form_submit".to_string(), 0.05),
                ("purchase".to_string(), 0.02),
                ("login".to_string(), 0.05),
                ("logout".to_string(), 0.03),
            ],
            events_per_session: EventCountModel::default(),
            session_duration_minutes: (1.0, 60.0),
        }
    }
}

impl EventConfig {
    /// E-commerce event types preset.
    pub fn ecommerce() -> Self {
        Self {
            event_types: vec![
                ("page_view".to_string(), 0.35),
                ("product_view".to_string(), 0.25),
                ("add_to_cart".to_string(), 0.15),
                ("remove_from_cart".to_string(), 0.05),
                ("checkout_start".to_string(), 0.10),
                ("purchase".to_string(), 0.05),
                ("search".to_string(), 0.05),
            ],
            events_per_session: EventCountModel::default(),
            session_duration_minutes: (2.0, 45.0),
        }
    }

    /// SaaS/app event types preset.
    pub fn saas() -> Self {
        Self {
            event_types: vec![
                ("page_view".to_string(), 0.25),
                ("feature_used".to_string(), 0.30),
                ("button_click".to_string(), 0.20),
                ("api_call".to_string(), 0.10),
                ("error".to_string(), 0.05),
                ("upgrade_prompt".to_string(), 0.05),
                ("settings_changed".to_string(), 0.05),
            ],
            events_per_session: EventCountModel::high_engagement(),
            session_duration_minutes: (5.0, 120.0),
        }
    }

    /// Media/content event types preset.
    pub fn media() -> Self {
        Self {
            event_types: vec![
                ("page_view".to_string(), 0.30),
                ("content_view".to_string(), 0.25),
                ("play".to_string(), 0.15),
                ("pause".to_string(), 0.10),
                ("share".to_string(), 0.05),
                ("like".to_string(), 0.08),
                ("comment".to_string(), 0.07),
            ],
            events_per_session: EventCountModel::default(),
            session_duration_minutes: (3.0, 90.0),
        }
    }
}

/// Schema configuration for generated tables.
#[derive(Clone)]
pub struct SchemaConfig {
    /// Name of the visitors table
    pub visitors_table: String,
    /// Name of the events table
    pub events_table: String,
    /// Name of the sessions table (optional)
    pub sessions_table: Option<String>,
    /// Database schema name
    pub db_schema: String,
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            visitors_table: "visitors".to_string(),
            events_table: "events".to_string(),
            sessions_table: Some("sessions".to_string()),
            db_schema: "testdata".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range_last_n_days() {
        let range = TimeRange::last_n_days(30);
        assert!(range.num_days() >= 29 && range.num_days() <= 30);
    }

    #[test]
    fn test_time_range_month() {
        let range = TimeRange::month(2024, 1);
        assert_eq!(range.num_days(), 30); // Jan 2024 has 31 days, but we subtract 1 second
    }

    #[test]
    fn test_default_config() {
        let config = TestDataConfig::default();
        assert_eq!(config.seed, 42);
        assert_eq!(config.visitors.count, 1000);
        assert!(!config.events.event_types.is_empty());
    }

    #[test]
    fn test_ecommerce_preset() {
        let config = EventConfig::ecommerce();
        let has_purchase = config
            .event_types
            .iter()
            .any(|(name, _)| name == "purchase");
        let has_add_to_cart = config
            .event_types
            .iter()
            .any(|(name, _)| name == "add_to_cart");
        assert!(has_purchase);
        assert!(has_add_to_cart);
    }
}
