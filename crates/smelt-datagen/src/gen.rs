//! Core generator trait and combinators.
//!
//! Inspired by proptest's Strategy trait, but simplified for data generation
//! without shrinking capability.

use rand::RngCore;

/// A generator that produces values of type `T` from a random source.
///
/// Generators are composable using methods like `map`, `flat_map`, and `filter`.
pub trait Gen<T> {
    /// Generate a value using the provided random source.
    fn generate(&self, rng: &mut dyn RngCore) -> T;

    /// Transform the generated value using a function.
    fn map<U, F>(self, f: F) -> Mapped<Self, F, T>
    where
        Self: Sized,
        F: Fn(T) -> U,
    {
        Mapped {
            gen: self,
            f,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Generate a value, then use it to create another generator.
    fn flat_map<U, G, F>(self, f: F) -> FlatMapped<Self, F, T, G>
    where
        Self: Sized,
        G: Gen<U>,
        F: Fn(T) -> G,
    {
        FlatMapped {
            gen: self,
            f,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Filter generated values, retrying until the predicate passes.
    fn filter<F>(self, predicate: F) -> Filtered<Self, F>
    where
        Self: Sized,
        F: Fn(&T) -> bool,
    {
        Filtered {
            gen: self,
            predicate,
        }
    }
}

/// A generator that applies a function to transform generated values.
pub struct Mapped<G, F, T> {
    gen: G,
    f: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U, G, F> Gen<U> for Mapped<G, F, T>
where
    G: Gen<T>,
    F: Fn(T) -> U,
{
    fn generate(&self, rng: &mut dyn RngCore) -> U {
        (self.f)(self.gen.generate(rng))
    }
}

/// A generator that chains generators based on generated values.
pub struct FlatMapped<G, F, T, H> {
    gen: G,
    f: F,
    _phantom: std::marker::PhantomData<(T, H)>,
}

impl<T, U, G, H, F> Gen<U> for FlatMapped<G, F, T, H>
where
    G: Gen<T>,
    H: Gen<U>,
    F: Fn(T) -> H,
{
    fn generate(&self, rng: &mut dyn RngCore) -> U {
        let inner = (self.f)(self.gen.generate(rng));
        inner.generate(rng)
    }
}

/// A generator that filters values based on a predicate.
pub struct Filtered<G, F> {
    gen: G,
    predicate: F,
}

impl<T, G, F> Gen<T> for Filtered<G, F>
where
    G: Gen<T>,
    F: Fn(&T) -> bool,
{
    fn generate(&self, rng: &mut dyn RngCore) -> T {
        loop {
            let value = self.gen.generate(rng);
            if (self.predicate)(&value) {
                return value;
            }
        }
    }
}
