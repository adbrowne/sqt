//! Pre-configured test data scenarios.
//!
//! These presets provide sensible defaults for common testing scenarios,
//! making it easy to get started without configuring every parameter.

use crate::builder::TestDataBuilder;
use crate::config::TestDataConfig;

/// Small dataset for unit tests.
///
/// - ~100 visitors
/// - ~7 days of data
/// - ~1,000 events
pub fn unit_test() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(100)
        .last_n_days(7)
        .build()
}

/// Medium dataset for integration tests.
///
/// - ~1,000 visitors
/// - ~30 days of data
/// - ~50,000 events
pub fn integration_test() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(1000)
        .last_n_days(30)
        .build()
}

/// Large dataset for performance testing.
///
/// - ~10,000 visitors
/// - ~90 days of data
/// - ~500,000 events
pub fn performance_test() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(10_000)
        .last_n_days(90)
        .build()
}

/// Very large dataset for stress testing.
///
/// - ~100,000 visitors
/// - ~365 days of data
/// - ~5,000,000 events
pub fn stress_test() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(100_000)
        .last_n_days(365)
        .build()
}

/// E-commerce scenario with purchase funnel events.
///
/// Includes: page_view, product_view, add_to_cart, checkout, purchase
pub fn ecommerce() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(5000)
        .last_n_days(30)
        .ecommerce_events()
        .build()
}

/// E-commerce scenario optimized for mobile.
pub fn ecommerce_mobile() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(5000)
        .last_n_days(30)
        .ecommerce_events()
        .mobile_first()
        .build()
}

/// SaaS application with feature usage events.
///
/// Includes: page_view, feature_used, button_click, api_call, error
pub fn saas() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(2000)
        .last_n_days(30)
        .saas_events()
        .high_engagement_visitors()
        .build()
}

/// Media/content site with engagement events.
///
/// Includes: page_view, content_view, play, pause, share, like, comment
pub fn media() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(3000)
        .last_n_days(30)
        .media_events()
        .build()
}

/// High churn scenario with many one-time visitors.
pub fn high_churn() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(10_000)
        .last_n_days(30)
        .low_engagement_visitors()
        .build()
}

/// Engaged user base with many power users.
pub fn high_engagement() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(2000)
        .last_n_days(30)
        .high_engagement_visitors()
        .build()
}

/// Web-first audience (primarily desktop/web users).
pub fn web_first() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(3000)
        .last_n_days(30)
        .web_first()
        .build()
}

/// Mobile-first audience (primarily iOS/Android users).
pub fn mobile_first() -> TestDataConfig {
    TestDataBuilder::new()
        .seed(42)
        .visitors(3000)
        .last_n_days(30)
        .mobile_first()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::TestDataGenerator;

    #[test]
    fn test_unit_test_preset() {
        let config = unit_test();
        assert_eq!(config.visitors.count, 100);
        assert_eq!(config.seed, 42);
    }

    #[test]
    fn test_integration_test_preset() {
        let config = integration_test();
        assert_eq!(config.visitors.count, 1000);
    }

    #[test]
    fn test_ecommerce_preset() {
        let config = ecommerce();
        let has_purchase = config
            .events
            .event_types
            .iter()
            .any(|(name, _)| name == "purchase");
        assert!(has_purchase);
    }

    #[test]
    fn test_presets_generate_data() {
        // Verify that all presets generate valid data
        let presets = vec![
            unit_test(),
            integration_test(),
            ecommerce(),
            saas(),
            media(),
            high_churn(),
            high_engagement(),
            web_first(),
            mobile_first(),
        ];

        for preset in presets {
            let generator = TestDataGenerator::new(preset);
            let data = generator.generate();
            assert!(!data.visitors.is_empty());
            assert!(!data.sessions.is_empty());
            assert!(!data.events.is_empty());
        }
    }
}
