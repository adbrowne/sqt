//! Core Generator trait and combinators for composable data generation.

use rand::rngs::StdRng;
use rand::Rng;
use std::marker::PhantomData;

/// Core trait for all data generators.
///
/// Unlike proptest's Strategy which focuses on shrinking for counterexamples,
/// Generator focuses on producing realistic volumes of data efficiently.
///
/// This trait uses StdRng as the RNG type to allow for dyn compatibility.
pub trait Generator<T>: Send + Sync {
    /// Generate a single value using the provided RNG.
    fn generate(&self, rng: &mut StdRng) -> T;

    /// Generate N values into a Vec (default implementation, can be optimized).
    fn generate_n(&self, rng: &mut StdRng, n: usize) -> Vec<T> {
        (0..n).map(|_| self.generate(rng)).collect()
    }
}

/// Extension trait providing combinator methods for generators.
pub trait GeneratorExt<T>: Generator<T> + Sized {
    /// Map the output to a different type.
    fn map<U, F: Fn(T) -> U + Send + Sync>(self, f: F) -> Map<Self, F, T> {
        Map {
            inner: self,
            f,
            _phantom: PhantomData,
        }
    }

    /// Filter and retry until predicate passes.
    fn filter<F: Fn(&T) -> bool + Send + Sync>(self, predicate: F) -> Filter<Self, F> {
        Filter {
            inner: self,
            predicate,
            max_retries: 100,
        }
    }

    /// Flat map - generate based on previous value.
    fn flat_map<U, G: Generator<U>, F: Fn(T) -> G + Send + Sync>(
        self,
        f: F,
    ) -> FlatMap<Self, F, G, T> {
        FlatMap {
            inner: self,
            f,
            _phantom: PhantomData,
        }
    }

    /// Box this generator for dynamic dispatch.
    fn boxed(self) -> BoxedGenerator<T>
    where
        Self: 'static,
        T: 'static,
    {
        Box::new(self)
    }
}

impl<T, G: Generator<T> + Sized> GeneratorExt<T> for G {}

/// Boxed generator for dynamic dispatch.
pub type BoxedGenerator<T> = Box<dyn Generator<T>>;

impl<T> Generator<T> for BoxedGenerator<T> {
    fn generate(&self, rng: &mut StdRng) -> T {
        (**self).generate(rng)
    }
}

// Also implement for &Box<dyn Generator<T>>
impl<T> Generator<T> for &BoxedGenerator<T> {
    fn generate(&self, rng: &mut StdRng) -> T {
        (**self).generate(rng)
    }
}

// ----------------------------------------------------------------------------
// Combinators
// ----------------------------------------------------------------------------

/// Map combinator - transforms generator output.
pub struct Map<G, F, T> {
    pub(crate) inner: G,
    pub(crate) f: F,
    pub(crate) _phantom: PhantomData<T>,
}

// Need to manually implement Send + Sync
unsafe impl<G: Send, F: Send, T> Send for Map<G, F, T> {}
unsafe impl<G: Sync, F: Sync, T> Sync for Map<G, F, T> {}

impl<T, U, G: Generator<T>, F: Fn(T) -> U + Send + Sync> Generator<U> for Map<G, F, T> {
    fn generate(&self, rng: &mut StdRng) -> U {
        (self.f)(self.inner.generate(rng))
    }
}

/// Filter combinator with retry limit.
pub struct Filter<G, F> {
    pub(crate) inner: G,
    pub(crate) predicate: F,
    pub(crate) max_retries: usize,
}

impl<T, G: Generator<T>, F: Fn(&T) -> bool + Send + Sync> Generator<T> for Filter<G, F> {
    fn generate(&self, rng: &mut StdRng) -> T {
        for _ in 0..self.max_retries {
            let value = self.inner.generate(rng);
            if (self.predicate)(&value) {
                return value;
            }
        }
        panic!("Filter exceeded max_retries ({})", self.max_retries);
    }
}

/// FlatMap combinator - chain generators together.
pub struct FlatMap<G, F, H, T> {
    pub(crate) inner: G,
    pub(crate) f: F,
    pub(crate) _phantom: PhantomData<(H, T)>,
}

// Need to manually implement Send + Sync
unsafe impl<G: Send, F: Send, H, T> Send for FlatMap<G, F, H, T> {}
unsafe impl<G: Sync, F: Sync, H, T> Sync for FlatMap<G, F, H, T> {}

impl<T, U, G, H, F> Generator<U> for FlatMap<G, F, H, T>
where
    G: Generator<T>,
    H: Generator<U>,
    F: Fn(T) -> H + Send + Sync,
{
    fn generate(&self, rng: &mut StdRng) -> U {
        let intermediate = self.inner.generate(rng);
        let next_gen = (self.f)(intermediate);
        next_gen.generate(rng)
    }
}

// ----------------------------------------------------------------------------
// Basic Generators
// ----------------------------------------------------------------------------

/// Generator from a closure.
pub struct ClosureGenerator<F> {
    f: F,
}

impl<F> ClosureGenerator<F> {
    pub fn new<T>(f: F) -> Self
    where
        F: Fn(&mut StdRng) -> T + Send + Sync,
    {
        Self { f }
    }
}

impl<T, F> Generator<T> for ClosureGenerator<F>
where
    F: Fn(&mut StdRng) -> T + Send + Sync,
{
    fn generate(&self, rng: &mut StdRng) -> T {
        (self.f)(rng)
    }
}

/// Helper function to create a closure-based generator.
pub fn gen<T, F>(f: F) -> ClosureGenerator<F>
where
    F: Fn(&mut StdRng) -> T + Send + Sync,
{
    ClosureGenerator::new(f)
}

/// Constant value generator.
#[derive(Clone)]
pub struct Constant<T>(pub T);

impl<T: Clone + Send + Sync> Generator<T> for Constant<T> {
    fn generate(&self, _rng: &mut StdRng) -> T {
        self.0.clone()
    }
}

/// Uniform u32 range generator.
pub struct UniformU32 {
    min: u32,
    max: u32,
}

impl UniformU32 {
    pub fn new(min: u32, max: u32) -> Self {
        Self { min, max }
    }
}

impl Generator<u32> for UniformU32 {
    fn generate(&self, rng: &mut StdRng) -> u32 {
        rng.gen_range(self.min..=self.max)
    }
}

/// Uniform f64 range generator.
pub struct UniformF64 {
    min: f64,
    max: f64,
}

impl UniformF64 {
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }
}

impl Generator<f64> for UniformF64 {
    fn generate(&self, rng: &mut StdRng) -> f64 {
        rng.gen_range(self.min..self.max)
    }
}

/// One of many generators, chosen by weight.
pub struct OneOf<T> {
    generators: Vec<(f64, BoxedGenerator<T>)>,
    total_weight: f64,
}

impl<T: 'static> OneOf<T> {
    pub fn new(generators: Vec<(f64, BoxedGenerator<T>)>) -> Self {
        let total_weight = generators.iter().map(|(w, _)| w).sum();
        Self {
            generators,
            total_weight,
        }
    }

    pub fn uniform(generators: Vec<BoxedGenerator<T>>) -> Self {
        let weight = 1.0;
        let weighted: Vec<_> = generators.into_iter().map(|g| (weight, g)).collect();
        Self::new(weighted)
    }
}

impl<T> Generator<T> for OneOf<T> {
    fn generate(&self, rng: &mut StdRng) -> T {
        let mut choice = rng.gen::<f64>() * self.total_weight;
        for (weight, gen) in &self.generators {
            choice -= weight;
            if choice <= 0.0 {
                return gen.generate(rng);
            }
        }
        self.generators.last().unwrap().1.generate(rng)
    }
}

/// Weighted choice from a list of values.
pub struct WeightedChoice<T> {
    choices: Vec<(T, f64)>,
    total_weight: f64,
}

impl<T: Clone + Send + Sync> WeightedChoice<T> {
    pub fn new(choices: Vec<(T, f64)>) -> Self {
        let total_weight = choices.iter().map(|(_, w)| w).sum();
        Self {
            choices,
            total_weight,
        }
    }
}

impl<T: Clone + Send + Sync> Generator<T> for WeightedChoice<T> {
    fn generate(&self, rng: &mut StdRng) -> T {
        let mut choice = rng.gen::<f64>() * self.total_weight;
        for (value, weight) in &self.choices {
            choice -= weight;
            if choice <= 0.0 {
                return value.clone();
            }
        }
        self.choices.last().unwrap().0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_constant_generator() {
        let gen = Constant(42);
        let mut rng = StdRng::seed_from_u64(0);
        assert_eq!(gen.generate(&mut rng), 42);
        assert_eq!(gen.generate(&mut rng), 42);
    }

    #[test]
    fn test_uniform_u32() {
        let gen = UniformU32::new(10, 20);
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..100 {
            let val = gen.generate(&mut rng);
            assert!((10..=20).contains(&val));
        }
    }

    #[test]
    fn test_map_combinator() {
        let gen = UniformU32::new(1, 10).map(|x| x * 2);
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..100 {
            let val = gen.generate(&mut rng);
            assert!((2..=20).contains(&val));
            assert!(val % 2 == 0);
        }
    }

    #[test]
    fn test_filter_combinator() {
        let gen = UniformU32::new(1, 100).filter(|x| x % 2 == 0);
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..100 {
            let val = gen.generate(&mut rng);
            assert!(val % 2 == 0);
        }
    }

    #[test]
    fn test_weighted_choice() {
        let gen = WeightedChoice::new(vec![("a".to_string(), 0.9), ("b".to_string(), 0.1)]);
        let mut rng = StdRng::seed_from_u64(0);
        let mut a_count = 0;
        let mut b_count = 0;
        for _ in 0..1000 {
            match gen.generate(&mut rng).as_str() {
                "a" => a_count += 1,
                "b" => b_count += 1,
                _ => unreachable!(),
            }
        }
        // With 90/10 weights, we expect roughly 900 a's and 100 b's
        assert!(a_count > 800, "Expected mostly 'a', got {}", a_count);
        assert!(b_count < 200, "Expected few 'b', got {}", b_count);
    }

    #[test]
    fn test_generate_n() {
        let gen = Constant(1);
        let mut rng = StdRng::seed_from_u64(0);
        let values = gen.generate_n(&mut rng, 5);
        assert_eq!(values, vec![1, 1, 1, 1, 1]);
    }

    #[test]
    fn test_determinism() {
        let gen = UniformU32::new(0, 1000);
        let mut rng1 = StdRng::seed_from_u64(42);
        let mut rng2 = StdRng::seed_from_u64(42);

        let values1: Vec<_> = (0..10).map(|_| gen.generate(&mut rng1)).collect();
        let values2: Vec<_> = (0..10).map(|_| gen.generate(&mut rng2)).collect();

        assert_eq!(values1, values2);
    }
}
