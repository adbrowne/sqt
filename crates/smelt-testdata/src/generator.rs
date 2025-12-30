//! Main test data generator.

use crate::config::TestDataConfig;
use crate::core::{Generator, WeightedChoice};
use crate::distributions::Platform;
use crate::rng::SeededRngFactory;
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use std::collections::HashMap;

/// A generated visitor.
#[derive(Debug, Clone)]
pub struct Visitor {
    /// Unique visitor identifier
    pub visitor_id: String,
    /// Platforms this visitor uses
    pub platforms: Vec<Platform>,
    /// Expected number of visits (sessions)
    pub expected_visits: u32,
    /// When this visitor first appeared
    pub first_seen: DateTime<Utc>,
}

/// A generated session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier
    pub session_id: String,
    /// Visitor who owns this session
    pub visitor_id: String,
    /// Platform used for this session
    pub platform: Platform,
    /// When the session started
    pub start_time: DateTime<Utc>,
    /// Duration of the session in minutes
    pub duration_minutes: f64,
}

/// A generated event.
#[derive(Debug, Clone)]
pub struct Event {
    /// Unique event identifier
    pub event_id: String,
    /// Session this event belongs to
    pub session_id: String,
    /// Visitor who triggered this event
    pub visitor_id: String,
    /// Type of event (e.g., "page_view", "click")
    pub event_type: String,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Platform on which the event occurred
    pub platform: Platform,
    /// Additional event properties
    pub properties: HashMap<String, String>,
}

/// All generated data.
#[derive(Debug)]
pub struct GeneratedData {
    /// Generated visitors
    pub visitors: Vec<Visitor>,
    /// Generated sessions
    pub sessions: Vec<Session>,
    /// Generated events
    pub events: Vec<Event>,
}

impl GeneratedData {
    /// Get a summary of the generated data.
    pub fn summary(&self) -> String {
        format!(
            "Generated {} visitors, {} sessions, {} events",
            self.visitors.len(),
            self.sessions.len(),
            self.events.len()
        )
    }
}

/// Main entry point for generating test data.
pub struct TestDataGenerator {
    config: TestDataConfig,
    rng_factory: SeededRngFactory,
}

impl TestDataGenerator {
    /// Create a new generator with the given configuration.
    pub fn new(config: TestDataConfig) -> Self {
        Self {
            rng_factory: SeededRngFactory::new(config.seed),
            config,
        }
    }

    /// Generate all test data: visitors, sessions, and events.
    pub fn generate(&self) -> GeneratedData {
        let visitors = self.generate_visitors();
        let sessions = self.generate_sessions(&visitors);
        let events = self.generate_events(&sessions);

        GeneratedData {
            visitors,
            sessions,
            events,
        }
    }

    /// Generate only visitors.
    pub fn generate_visitors(&self) -> Vec<Visitor> {
        let mut rng = self.rng_factory.stream("visitors");
        let period_days = self.config.time_range.num_days() as u32;
        let frequency_gen = self
            .config
            .visitors
            .frequency_model
            .visit_count_generator(period_days);

        (0..self.config.visitors.count)
            .map(|i| {
                let platforms = self
                    .config
                    .visitors
                    .platform_model
                    .generate_platforms(&mut rng);
                let expected_visits = frequency_gen.generate(&mut rng);
                let first_seen = self.random_time_in_range(&mut rng);

                Visitor {
                    visitor_id: format!("v_{:08x}", i),
                    platforms,
                    expected_visits,
                    first_seen,
                }
            })
            .collect()
    }

    /// Generate sessions for the given visitors.
    pub fn generate_sessions(&self, visitors: &[Visitor]) -> Vec<Session> {
        let mut rng = self.rng_factory.stream("sessions");
        let mut sessions = Vec::new();
        let mut session_counter = 0u64;

        for visitor in visitors {
            // Generate sessions spread across the time range after first_seen
            let time_available = self.config.time_range.end - visitor.first_seen;
            let time_available_ms = time_available.num_milliseconds().max(1);

            for _ in 0..visitor.expected_visits {
                // Pick a random platform from the visitor's platforms
                let platform_idx = rng.gen_range(0..visitor.platforms.len());
                let platform = visitor.platforms[platform_idx].clone();

                // Pick a random time after first_seen
                let offset_ms = rng.gen_range(0..time_available_ms);
                let start_time = visitor.first_seen + Duration::milliseconds(offset_ms);

                // Generate session duration
                let (min_dur, max_dur) = self.config.events.session_duration_minutes;
                let duration_minutes = rng.gen_range(min_dur..max_dur);

                sessions.push(Session {
                    session_id: format!("s_{:012x}", session_counter),
                    visitor_id: visitor.visitor_id.clone(),
                    platform,
                    start_time,
                    duration_minutes,
                });

                session_counter += 1;
            }
        }

        // Sort sessions by start time for more realistic output
        sessions.sort_by_key(|s| s.start_time);

        sessions
    }

    /// Generate events for the given sessions.
    pub fn generate_events(&self, sessions: &[Session]) -> Vec<Event> {
        let mut rng = self.rng_factory.stream("events");
        let events_per_session = self.config.events.events_per_session.generator();
        let event_type_gen = WeightedChoice::new(self.config.events.event_types.clone());

        let mut events = Vec::new();
        let mut event_counter = 0u64;

        for session in sessions {
            let num_events = events_per_session.generate(&mut rng);
            let session_duration_ms = (session.duration_minutes * 60.0 * 1000.0) as i64;

            for i in 0..num_events {
                // Spread events across session duration
                let offset_ratio = if num_events > 1 {
                    i as f64 / (num_events - 1) as f64
                } else {
                    0.0
                };
                let offset_ms = (session_duration_ms as f64 * offset_ratio) as i64;
                // Add some jitter
                let jitter = if session_duration_ms > 0 {
                    rng.gen_range(0..session_duration_ms.min(1000))
                } else {
                    0
                };
                let timestamp = session.start_time + Duration::milliseconds(offset_ms + jitter);

                events.push(Event {
                    event_id: format!("e_{:016x}", event_counter),
                    session_id: session.session_id.clone(),
                    visitor_id: session.visitor_id.clone(),
                    event_type: event_type_gen.generate(&mut rng),
                    timestamp,
                    platform: session.platform.clone(),
                    properties: HashMap::new(),
                });

                event_counter += 1;
            }
        }

        // Sort events by timestamp for more realistic output
        events.sort_by_key(|e| e.timestamp);

        events
    }

    fn random_time_in_range<R: Rng>(&self, rng: &mut R) -> DateTime<Utc> {
        let range_millis = self.config.time_range.duration().num_milliseconds();
        if range_millis <= 0 {
            return self.config.time_range.start;
        }
        let offset = rng.gen_range(0..range_millis);
        self.config.time_range.start + Duration::milliseconds(offset)
    }

    /// Get the configuration used by this generator.
    pub fn config(&self) -> &TestDataConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::TestDataBuilder;

    #[test]
    fn test_generate_visitors() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(100)
            .last_n_days(30)
            .build();

        let generator = TestDataGenerator::new(config);
        let visitors = generator.generate_visitors();

        assert_eq!(visitors.len(), 100);
        for visitor in &visitors {
            assert!(!visitor.platforms.is_empty());
            assert!(visitor.expected_visits >= 1);
        }
    }

    #[test]
    fn test_generate_sessions() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(10)
            .last_n_days(30)
            .build();

        let generator = TestDataGenerator::new(config);
        let visitors = generator.generate_visitors();
        let sessions = generator.generate_sessions(&visitors);

        // Should have at least as many sessions as visitors (each has >= 1 visit)
        assert!(sessions.len() >= visitors.len());

        // Each session should reference a valid visitor
        for session in &sessions {
            let visitor = visitors.iter().find(|v| v.visitor_id == session.visitor_id);
            assert!(visitor.is_some());

            // Platform should be one the visitor uses
            let v = visitor.unwrap();
            assert!(v.platforms.contains(&session.platform));
        }
    }

    #[test]
    fn test_generate_events() {
        let config = TestDataBuilder::new()
            .seed(42)
            .visitors(10)
            .last_n_days(30)
            .build();

        let generator = TestDataGenerator::new(config);
        let data = generator.generate();

        // Should have events
        assert!(!data.events.is_empty());

        // Events should be sorted by timestamp
        for window in data.events.windows(2) {
            assert!(window[0].timestamp <= window[1].timestamp);
        }

        // Each event should reference a valid session
        for event in &data.events {
            let session = data
                .sessions
                .iter()
                .find(|s| s.session_id == event.session_id);
            assert!(session.is_some());
        }
    }

    #[test]
    fn test_determinism() {
        let config1 = TestDataBuilder::new()
            .seed(42)
            .visitors(100)
            .last_n_days(30)
            .build();

        let config2 = TestDataBuilder::new()
            .seed(42)
            .visitors(100)
            .last_n_days(30)
            .build();

        let gen1 = TestDataGenerator::new(config1);
        let gen2 = TestDataGenerator::new(config2);

        let data1 = gen1.generate();
        let data2 = gen2.generate();

        assert_eq!(data1.visitors.len(), data2.visitors.len());
        assert_eq!(data1.sessions.len(), data2.sessions.len());
        assert_eq!(data1.events.len(), data2.events.len());

        // Check that visitor IDs match
        for (v1, v2) in data1.visitors.iter().zip(data2.visitors.iter()) {
            assert_eq!(v1.visitor_id, v2.visitor_id);
            assert_eq!(v1.expected_visits, v2.expected_visits);
        }
    }

    #[test]
    fn test_different_seeds_produce_different_data() {
        let config1 = TestDataBuilder::new().seed(42).visitors(100).build();
        let config2 = TestDataBuilder::new().seed(43).visitors(100).build();

        let gen1 = TestDataGenerator::new(config1);
        let gen2 = TestDataGenerator::new(config2);

        let data1 = gen1.generate();
        let data2 = gen2.generate();

        // Should have same counts but different content
        assert_eq!(data1.visitors.len(), data2.visitors.len());

        // Check that at least some visitor attributes differ
        let different_visits = data1
            .visitors
            .iter()
            .zip(data2.visitors.iter())
            .any(|(v1, v2)| v1.expected_visits != v2.expected_visits);

        assert!(different_visits);
    }
}
