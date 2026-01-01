//! Built-in generators for common types.

use crate::gen::Gen;
use rand::distributions::{Distribution, WeightedIndex};
use rand::RngCore;
use std::ops::Range;
use uuid::Uuid;

/// Generate a value uniformly distributed in the given range.
pub struct Uniform<T> {
    range: Range<T>,
}

impl<T> Uniform<T> {
    pub fn new(range: Range<T>) -> Self {
        Self { range }
    }
}

impl Gen<i32> for Uniform<i32> {
    fn generate(&self, rng: &mut dyn RngCore) -> i32 {
        use rand::distributions::Uniform as UniformDist;
        let dist = UniformDist::new(self.range.start, self.range.end);
        dist.sample(rng)
    }
}

impl Gen<i64> for Uniform<i64> {
    fn generate(&self, rng: &mut dyn RngCore) -> i64 {
        use rand::distributions::Uniform as UniformDist;
        let dist = UniformDist::new(self.range.start, self.range.end);
        dist.sample(rng)
    }
}

impl Gen<u32> for Uniform<u32> {
    fn generate(&self, rng: &mut dyn RngCore) -> u32 {
        use rand::distributions::Uniform as UniformDist;
        let dist = UniformDist::new(self.range.start, self.range.end);
        dist.sample(rng)
    }
}

impl Gen<u64> for Uniform<u64> {
    fn generate(&self, rng: &mut dyn RngCore) -> u64 {
        use rand::distributions::Uniform as UniformDist;
        let dist = UniformDist::new(self.range.start, self.range.end);
        dist.sample(rng)
    }
}

impl Gen<f64> for Uniform<f64> {
    fn generate(&self, rng: &mut dyn RngCore) -> f64 {
        use rand::distributions::Uniform as UniformDist;
        let dist = UniformDist::new(self.range.start, self.range.end);
        dist.sample(rng)
    }
}

impl Gen<usize> for Uniform<usize> {
    fn generate(&self, rng: &mut dyn RngCore) -> usize {
        use rand::distributions::Uniform as UniformDist;
        let dist = UniformDist::new(self.range.start, self.range.end);
        dist.sample(rng)
    }
}

/// Convenience function to create a uniform generator.
pub fn uniform<T>(range: Range<T>) -> Uniform<T> {
    Uniform::new(range)
}

/// Generate a value selected from weighted choices.
pub struct WeightedChoice<T> {
    items: Vec<T>,
    weights: WeightedIndex<f64>,
}

impl<T: Clone> WeightedChoice<T> {
    pub fn new(items: Vec<(T, f64)>) -> Self {
        let (items, weights): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let weights = WeightedIndex::new(&weights).expect("weights must be positive");
        Self { items, weights }
    }
}

impl<T: Clone> Gen<T> for WeightedChoice<T> {
    fn generate(&self, rng: &mut dyn RngCore) -> T {
        let idx = self.weights.sample(rng);
        self.items[idx].clone()
    }
}

/// Convenience function to create a weighted choice generator.
pub fn weighted_choice<T: Clone>(items: Vec<(T, f64)>) -> WeightedChoice<T> {
    WeightedChoice::new(items)
}

/// Generate a value uniformly selected from a slice.
pub struct OneOf<T> {
    items: Vec<T>,
}

impl<T: Clone> OneOf<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }
}

impl<T: Clone> Gen<T> for OneOf<T> {
    fn generate(&self, rng: &mut dyn RngCore) -> T {
        let idx = rng.next_u64() as usize % self.items.len();
        self.items[idx].clone()
    }
}

/// Convenience function to create a one-of generator.
pub fn one_of<T: Clone>(items: Vec<T>) -> OneOf<T> {
    OneOf::new(items)
}

/// Generate a deterministic UUID from random bytes.
pub struct UuidGen;

impl Gen<Uuid> for UuidGen {
    fn generate(&self, rng: &mut dyn RngCore) -> Uuid {
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        // Set version 4 (random) bits
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        // Set variant bits
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes)
    }
}

/// Convenience function to create a UUID generator.
pub fn uuid_gen() -> UuidGen {
    UuidGen
}

/// Generate a boolean with the given probability of being true.
pub struct BoolWithProb {
    prob: f64,
}

impl BoolWithProb {
    pub fn new(prob: f64) -> Self {
        Self { prob }
    }
}

impl Gen<bool> for BoolWithProb {
    fn generate(&self, rng: &mut dyn RngCore) -> bool {
        // Generate a random f64 in [0, 1) and compare to probability
        let r = (rng.next_u64() as f64) / (u64::MAX as f64);
        r < self.prob
    }
}

/// Convenience function to create a boolean generator with given probability.
pub fn bool_with_prob(prob: f64) -> BoolWithProb {
    BoolWithProb::new(prob)
}

/// Generate a constant value.
pub struct Constant<T> {
    value: T,
}

impl<T: Clone> Constant<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T: Clone> Gen<T> for Constant<T> {
    fn generate(&self, _rng: &mut dyn RngCore) -> T {
        self.value.clone()
    }
}

/// Convenience function to create a constant generator.
pub fn constant<T: Clone>(value: T) -> Constant<T> {
    Constant::new(value)
}

/// Generate an optional value with the given probability of being Some.
pub struct Optional<G, T> {
    gen: G,
    some_prob: f64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, G: Gen<T>> Optional<G, T> {
    pub fn new(gen: G, some_prob: f64) -> Self {
        Self {
            gen,
            some_prob,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, G: Gen<T>> Gen<Option<T>> for Optional<G, T> {
    fn generate(&self, rng: &mut dyn RngCore) -> Option<T> {
        let r = (rng.next_u64() as f64) / (u64::MAX as f64);
        if r < self.some_prob {
            Some(self.gen.generate(rng))
        } else {
            None
        }
    }
}

/// Convenience function to create an optional generator.
pub fn optional<T, G: Gen<T>>(gen: G, some_prob: f64) -> Optional<G, T> {
    Optional::new(gen, some_prob)
}

/// Generate a log-normal distributed integer.
/// Useful for things like page views, session duration, etc.
pub struct LogNormal {
    mu: f64,
    sigma: f64,
    max: i32,
}

impl LogNormal {
    pub fn new(median: f64, sigma: f64, max: i32) -> Self {
        // For log-normal, median = e^mu, so mu = ln(median)
        let mu = median.ln();
        Self { mu, sigma, max }
    }
}

impl Gen<i32> for LogNormal {
    fn generate(&self, rng: &mut dyn RngCore) -> i32 {
        use rand_distr::{Distribution, LogNormal as LogNormalDist};
        let dist = LogNormalDist::new(self.mu, self.sigma).unwrap();
        let value = dist.sample(rng) as i32;
        value.min(self.max)
    }
}

/// Convenience function to create a log-normal generator.
pub fn log_normal(median: f64, sigma: f64, max: i32) -> LogNormal {
    LogNormal::new(median, sigma, max)
}

/// Generate values from a geometric distribution.
/// Useful for counts that follow "number of tries until success" pattern.
pub struct Geometric {
    p: f64,
}

impl Geometric {
    pub fn new(p: f64) -> Self {
        Self { p }
    }
}

impl Gen<i32> for Geometric {
    fn generate(&self, rng: &mut dyn RngCore) -> i32 {
        use rand_distr::{Distribution, Geometric as GeomDist};
        let dist = GeomDist::new(self.p).unwrap();
        dist.sample(rng) as i32
    }
}

/// Convenience function to create a geometric generator.
pub fn geometric(p: f64) -> Geometric {
    Geometric::new(p)
}
