//! Statistical distribution models for realistic data generation.

use crate::core::{BoxedGenerator, Constant, Generator, GeneratorExt, OneOf, UniformU32};
use rand::rngs::StdRng;
use rand::Rng;
use rand_distr::{Distribution, Pareto};

// ----------------------------------------------------------------------------
// Power Law Distribution
// ----------------------------------------------------------------------------

/// Power law distribution for "long tail" patterns.
///
/// Perfect for:
/// - Visitor frequency (few power users, many occasional visitors)
/// - Event counts per visitor
/// - Page popularity
pub struct PowerLaw {
    pareto: Pareto<f64>,
    max_value: Option<f64>,
}

impl PowerLaw {
    /// Create a power law distribution.
    ///
    /// - `alpha`: Shape parameter (higher = steeper drop-off)
    ///   - 1.0-1.5: Very heavy tail (social media followers)
    ///   - 2.0-2.5: Moderate tail (website visits)
    ///   - 3.0+: Light tail (closer to normal)
    /// - `min_value`: Minimum possible value (scale parameter)
    pub fn new(alpha: f64, min_value: f64) -> Self {
        Self {
            pareto: Pareto::new(min_value, alpha).expect("Invalid Pareto parameters"),
            max_value: None,
        }
    }

    /// Set a maximum value (values above this are capped).
    pub fn with_max(mut self, max: f64) -> Self {
        self.max_value = Some(max);
        self
    }
}

impl Generator<f64> for PowerLaw {
    fn generate(&self, rng: &mut StdRng) -> f64 {
        let value = self.pareto.sample(rng);
        match self.max_value {
            Some(max) => value.min(max),
            None => value,
        }
    }
}

// ----------------------------------------------------------------------------
// Platform Model
// ----------------------------------------------------------------------------

/// Supported platforms for visitor events.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    Web,
    IOS,
    Android,
    API,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Web => "web",
            Platform::IOS => "ios",
            Platform::Android => "android",
            Platform::API => "api",
        }
    }
}

/// Platform affinity model.
///
/// Models that some visitors stick to one platform while others use multiple.
#[derive(Clone)]
pub struct PlatformAffinityModel {
    /// Probability of using only one platform
    pub single_platform_prob: f64,
    /// Probability of using exactly two platforms
    pub dual_platform_prob: f64,
    /// Base weights for each platform (name, weight)
    pub platform_weights: Vec<(Platform, f64)>,
}

impl Default for PlatformAffinityModel {
    fn default() -> Self {
        Self {
            single_platform_prob: 0.70, // 70% use only one platform
            dual_platform_prob: 0.25,   // 25% use exactly two
            // Remaining 5% use 3+ platforms
            platform_weights: vec![
                (Platform::Web, 0.40),
                (Platform::IOS, 0.30),
                (Platform::Android, 0.25),
                (Platform::API, 0.05),
            ],
        }
    }
}

impl PlatformAffinityModel {
    /// Generate the set of platforms a visitor uses.
    pub fn generate_platforms(&self, rng: &mut StdRng) -> Vec<Platform> {
        let roll: f64 = rng.gen();

        let num_platforms = if roll < self.single_platform_prob {
            1
        } else if roll < self.single_platform_prob + self.dual_platform_prob {
            2
        } else {
            rng.gen_range(3..=self.platform_weights.len())
        };

        // Weighted sample without replacement
        let mut available: Vec<_> = self.platform_weights.clone();
        let mut selected = Vec::new();

        for _ in 0..num_platforms.min(available.len()) {
            let total: f64 = available.iter().map(|(_, w)| w).sum();
            if total <= 0.0 {
                break;
            }

            let mut choice = rng.gen::<f64>() * total;

            let idx = available
                .iter()
                .position(|(_, w)| {
                    choice -= w;
                    choice <= 0.0
                })
                .unwrap_or(0);

            selected.push(available.remove(idx).0);
        }

        selected
    }

    /// Create a mobile-heavy platform model.
    pub fn mobile_first() -> Self {
        Self {
            single_platform_prob: 0.80,
            dual_platform_prob: 0.15,
            platform_weights: vec![
                (Platform::IOS, 0.45),
                (Platform::Android, 0.45),
                (Platform::Web, 0.08),
                (Platform::API, 0.02),
            ],
        }
    }

    /// Create a web-heavy platform model.
    pub fn web_first() -> Self {
        Self {
            single_platform_prob: 0.75,
            dual_platform_prob: 0.20,
            platform_weights: vec![
                (Platform::Web, 0.70),
                (Platform::IOS, 0.15),
                (Platform::Android, 0.12),
                (Platform::API, 0.03),
            ],
        }
    }
}

// ----------------------------------------------------------------------------
// Visitor Frequency Model
// ----------------------------------------------------------------------------

/// Visitor frequency model based on configurable user segments.
#[derive(Clone)]
pub struct VisitorFrequencyModel {
    /// Weight for power users (visit frequently)
    pub power_user_weight: f64,
    /// Weight for regular users (visit weekly)
    pub regular_weight: f64,
    /// Weight for occasional users (visit monthly)
    pub occasional_weight: f64,
    /// Weight for one-time users (single visit)
    pub one_time_weight: f64,
}

impl Default for VisitorFrequencyModel {
    fn default() -> Self {
        Self {
            power_user_weight: 0.05, // 5% are power users
            regular_weight: 0.20,    // 20% regular
            occasional_weight: 0.35, // 35% occasional
            one_time_weight: 0.40,   // 40% one-time
        }
    }
}

impl VisitorFrequencyModel {
    /// Generate visit count for a time period.
    ///
    /// Returns a generator that produces visit counts based on user segment.
    pub fn visit_count_generator(&self, period_days: u32) -> OneOf<u32> {
        let power_user_min = (period_days as f64 * 0.5) as u32;
        let power_user_max = (period_days as f64 * 0.9) as u32;

        let regular_min = (period_days as f64 / 7.0).ceil() as u32;
        let regular_max = (period_days as f64 / 7.0 * 2.0).ceil() as u32;

        let occasional_min = 2;
        let occasional_max = (period_days as f64 / 14.0).ceil() as u32;

        OneOf::new(vec![
            (
                self.power_user_weight,
                Box::new(UniformU32::new(
                    power_user_min.max(1),
                    power_user_max.max(power_user_min + 1),
                )) as BoxedGenerator<u32>,
            ),
            (
                self.regular_weight,
                Box::new(UniformU32::new(
                    regular_min.max(1),
                    regular_max.max(regular_min + 1),
                )) as BoxedGenerator<u32>,
            ),
            (
                self.occasional_weight,
                Box::new(UniformU32::new(
                    occasional_min,
                    occasional_max.max(occasional_min + 1),
                )) as BoxedGenerator<u32>,
            ),
            (
                self.one_time_weight,
                Box::new(Constant(1u32)) as BoxedGenerator<u32>,
            ),
        ])
    }

    /// Create a model for highly engaged users.
    pub fn high_engagement() -> Self {
        Self {
            power_user_weight: 0.20,
            regular_weight: 0.40,
            occasional_weight: 0.30,
            one_time_weight: 0.10,
        }
    }

    /// Create a model for low engagement / high churn.
    pub fn low_engagement() -> Self {
        Self {
            power_user_weight: 0.02,
            regular_weight: 0.10,
            occasional_weight: 0.28,
            one_time_weight: 0.60,
        }
    }
}

// ----------------------------------------------------------------------------
// Event Count Model
// ----------------------------------------------------------------------------

/// Events per session distribution model.
///
/// Models that some visitors generate many events (deep engagement)
/// while others generate few (quick bounces).
#[derive(Clone)]
pub struct EventCountModel {
    /// Power law shape parameter (higher = fewer high-event sessions)
    pub alpha: f64,
    /// Minimum events per session
    pub min_events: u32,
    /// Maximum events per session
    pub max_events: u32,
}

impl Default for EventCountModel {
    fn default() -> Self {
        Self {
            alpha: 2.0,
            min_events: 1,
            max_events: 100,
        }
    }
}

impl EventCountModel {
    /// Create an event count generator.
    pub fn generator(&self) -> impl Generator<u32> {
        let alpha = self.alpha;
        let min = self.min_events;
        let max = self.max_events;

        PowerLaw::new(alpha, min as f64)
            .with_max(max as f64)
            .map(|f| f as u32)
    }

    /// Model for high-engagement sessions (many events).
    pub fn high_engagement() -> Self {
        Self {
            alpha: 1.5,
            min_events: 3,
            max_events: 200,
        }
    }

    /// Model for quick sessions (few events).
    pub fn quick_sessions() -> Self {
        Self {
            alpha: 3.0,
            min_events: 1,
            max_events: 20,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_power_law_respects_min() {
        let gen = PowerLaw::new(2.0, 5.0);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        for _ in 0..100 {
            let val = gen.generate(&mut rng);
            assert!(val >= 5.0, "Value {} is less than min 5.0", val);
        }
    }

    #[test]
    fn test_power_law_respects_max() {
        let gen = PowerLaw::new(2.0, 1.0).with_max(10.0);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        for _ in 0..100 {
            let val = gen.generate(&mut rng);
            assert!(val <= 10.0, "Value {} exceeds max 10.0", val);
        }
    }

    #[test]
    fn test_platform_affinity_single_platform() {
        let model = PlatformAffinityModel {
            single_platform_prob: 1.0,
            dual_platform_prob: 0.0,
            platform_weights: vec![(Platform::Web, 0.5), (Platform::IOS, 0.5)],
        };

        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let platforms = model.generate_platforms(&mut rng);
            assert_eq!(platforms.len(), 1);
        }
    }

    #[test]
    fn test_platform_affinity_dual_platform() {
        let model = PlatformAffinityModel {
            single_platform_prob: 0.0,
            dual_platform_prob: 1.0,
            platform_weights: vec![
                (Platform::Web, 0.33),
                (Platform::IOS, 0.33),
                (Platform::Android, 0.34),
            ],
        };

        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let platforms = model.generate_platforms(&mut rng);
            assert_eq!(platforms.len(), 2);
        }
    }

    #[test]
    fn test_visitor_frequency_one_time() {
        let model = VisitorFrequencyModel {
            power_user_weight: 0.0,
            regular_weight: 0.0,
            occasional_weight: 0.0,
            one_time_weight: 1.0,
        };

        let gen = model.visit_count_generator(30);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        for _ in 0..100 {
            let count = gen.generate(&mut rng);
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn test_visitor_frequency_power_user() {
        let model = VisitorFrequencyModel {
            power_user_weight: 1.0,
            regular_weight: 0.0,
            occasional_weight: 0.0,
            one_time_weight: 0.0,
        };

        let gen = model.visit_count_generator(30);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        for _ in 0..100 {
            let count = gen.generate(&mut rng);
            assert!(count >= 15, "Power user should visit often, got {}", count);
        }
    }

    #[test]
    fn test_event_count_model() {
        let model = EventCountModel::default();
        let gen = model.generator();
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        for _ in 0..100 {
            let count = gen.generate(&mut rng);
            assert!(count >= 1);
            assert!(count <= 100);
        }
    }
}
