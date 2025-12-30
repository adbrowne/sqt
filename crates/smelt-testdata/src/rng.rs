//! Seeded RNG management for reproducible data generation.

use rand::{rngs::StdRng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A reproducible RNG factory that creates child RNGs from a master seed.
///
/// This allows:
/// 1. Full reproducibility from a single seed
/// 2. Independent streams for different entity types
/// 3. Stable output even when generation order changes
///
/// # Example
/// ```
/// use smelt_testdata::rng::SeededRngFactory;
///
/// let factory = SeededRngFactory::new(42);
///
/// // Named streams are independent and reproducible
/// let mut visitor_rng = factory.stream("visitors");
/// let mut event_rng = factory.stream("events");
///
/// // Same name always produces same sequence
/// let mut visitor_rng2 = factory.stream("visitors");
/// // visitor_rng and visitor_rng2 will produce the same sequence
/// ```
pub struct SeededRngFactory {
    master_seed: u64,
}

impl SeededRngFactory {
    /// Create a new RNG factory with the given master seed.
    pub fn new(seed: u64) -> Self {
        Self { master_seed: seed }
    }

    /// Create a child RNG for a named stream (e.g., "visitors", "events").
    ///
    /// Same name always produces the same sequence, regardless of the order
    /// in which streams are created.
    pub fn stream(&self, name: &str) -> StdRng {
        let mut hasher = DefaultHasher::new();
        self.master_seed.hash(&mut hasher);
        name.hash(&mut hasher);
        StdRng::seed_from_u64(hasher.finish())
    }

    /// Create a child RNG for a named stream with an additional index.
    ///
    /// Useful for generating independent streams for each entity.
    pub fn indexed_stream(&self, name: &str, index: u64) -> StdRng {
        let mut hasher = DefaultHasher::new();
        self.master_seed.hash(&mut hasher);
        name.hash(&mut hasher);
        index.hash(&mut hasher);
        StdRng::seed_from_u64(hasher.finish())
    }

    /// Get the master seed.
    pub fn seed(&self) -> u64 {
        self.master_seed
    }
}

impl Default for SeededRngFactory {
    fn default() -> Self {
        Self::new(42)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_same_seed_same_sequence() {
        let factory1 = SeededRngFactory::new(42);
        let factory2 = SeededRngFactory::new(42);

        let mut rng1 = factory1.stream("test");
        let mut rng2 = factory2.stream("test");

        let values1: Vec<u32> = (0..10).map(|_| rng1.gen()).collect();
        let values2: Vec<u32> = (0..10).map(|_| rng2.gen()).collect();

        assert_eq!(values1, values2);
    }

    #[test]
    fn test_different_streams_different_sequence() {
        let factory = SeededRngFactory::new(42);

        let mut visitors = factory.stream("visitors");
        let mut events = factory.stream("events");

        let visitor_values: Vec<u32> = (0..10).map(|_| visitors.gen()).collect();
        let event_values: Vec<u32> = (0..10).map(|_| events.gen()).collect();

        assert_ne!(visitor_values, event_values);
    }

    #[test]
    fn test_stream_order_independence() {
        let factory1 = SeededRngFactory::new(42);
        let factory2 = SeededRngFactory::new(42);

        // Create streams in different order
        let mut visitors1 = factory1.stream("visitors");
        let _events1 = factory1.stream("events");

        let _events2 = factory2.stream("events");
        let mut visitors2 = factory2.stream("visitors");

        // Should still produce same sequences
        let values1: Vec<u32> = (0..10).map(|_| visitors1.gen()).collect();
        let values2: Vec<u32> = (0..10).map(|_| visitors2.gen()).collect();

        assert_eq!(values1, values2);
    }

    #[test]
    fn test_indexed_streams() {
        let factory = SeededRngFactory::new(42);

        let mut stream0 = factory.indexed_stream("visitor", 0);
        let mut stream1 = factory.indexed_stream("visitor", 1);

        let values0: Vec<u32> = (0..10).map(|_| stream0.gen()).collect();
        let values1: Vec<u32> = (0..10).map(|_| stream1.gen()).collect();

        assert_ne!(values0, values1);
    }

    #[test]
    fn test_different_seeds_different_sequence() {
        let factory1 = SeededRngFactory::new(42);
        let factory2 = SeededRngFactory::new(43);

        let mut rng1 = factory1.stream("test");
        let mut rng2 = factory2.stream("test");

        let values1: Vec<u32> = (0..10).map(|_| rng1.gen()).collect();
        let values2: Vec<u32> = (0..10).map(|_| rng2.gen()).collect();

        assert_ne!(values1, values2);
    }
}
