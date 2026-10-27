use crate::api::{BestSoFar, Particle, ParticleView, Point};
use rand::RngCore;
use std::marker::PhantomData;

// The traits below are the machinery behind the public constructors. Users
// write closures and compose the values built from them, and never name
// these traits.

/// What an objective provides: for an outer position, a scoring function.
pub trait Objective: Send + Sync {
    type Pos: Point;
    type Outer: Copy;
    type Prepared<'a>: Fn(Self::Pos) -> f64 + Sync
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: Self::Outer) -> Self::Prepared<'a>;
}

/// What a region provides: for an outer position, a confining function.
pub trait Region: Send + Sync {
    type Pos: Point;
    type Outer: Copy;
    type Prepared<'a>: Fn(Self::Pos) -> Self::Pos + Sync
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: Self::Outer) -> Self::Prepared<'a>;
}

/// A rule for particles of position `P`, in a search whose outer position
/// is `O`.
pub trait Rule<P: Point, O>: Send + Sync {
    type Prepared<'a>: PreparedRule<P>
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: O) -> Self::Prepared<'a>;
}

/// A rule ready to move particles of one search.
pub trait PreparedRule<P: Point>: Sync {
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>);
}

/// Makes the particles an inner search starts from.
pub trait Seeder: Send + Sync {
    type Outer: Copy;
    type Inner: Point;

    fn seed(&self, outer: Self::Outer, rng: &mut dyn RngCore) -> Vec<Particle<Self::Inner>>;
}

macro_rules! closure_component {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        pub struct $name<F, O, P, G> {
            f: F,
            _marker: PhantomData<fn(O) -> (P, G)>,
        }

        impl<F: Clone, O, P, G> Clone for $name<F, O, P, G> {
            fn clone(&self) -> Self {
                $name { f: self.f.clone(), _marker: PhantomData }
            }
        }
    };
}

closure_component!(
    /// An objective built from a closure.
    FnObjective
);
closure_component!(
    /// A region built from a closure.
    FnRegion
);
closure_component!(
    /// A rule built from a closure.
    FnRule
);

/// A seeder built from a closure.
pub struct FnSeeder<F, O, I> {
    f: F,
    _marker: PhantomData<fn(O) -> I>,
}

impl<F: Clone, O, I> Clone for FnSeeder<F, O, I> {
    fn clone(&self) -> Self {
        FnSeeder { f: self.f.clone(), _marker: PhantomData }
    }
}

/// An objective: `f(outer)` returns the function that scores positions.
pub fn objective<O, P, G, F>(f: F) -> FnObjective<F, O, P, G>
where
    O: Copy,
    P: Point,
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> f64 + Sync,
{
    FnObjective { f, _marker: PhantomData }
}

/// A region: `f(outer)` returns the function that maps a position back into
/// the feasible region.
pub fn region<O, P, G, F>(f: F) -> FnRegion<F, O, P, G>
where
    O: Copy,
    P: Point,
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> P + Sync,
{
    FnRegion { f, _marker: PhantomData }
}

/// A rule: `f(outer)` returns the function that moves one particle.
pub fn rule<O, P, G, F>(f: F) -> FnRule<F, O, P, G>
where
    O: Copy,
    P: Point,
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Sync,
{
    FnRule { f, _marker: PhantomData }
}

/// A seeder: `f(outer, rng)` returns the particles an inner search starts from.
pub fn seeder<O, I, F>(f: F) -> FnSeeder<F, O, I>
where
    O: Copy,
    I: Point,
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>> + Clone + Send + Sync,
{
    FnSeeder { f, _marker: PhantomData }
}

impl<F, O, P, G> Objective for FnObjective<F, O, P, G>
where
    O: Copy,
    P: Point,
    F: Fn(O) -> G + Send + Sync,
    G: Fn(P) -> f64 + Sync,
{
    type Pos = P;
    type Outer = O;
    type Prepared<'a>
        = G
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: O) -> G {
        (self.f)(outer)
    }
}

impl<F, O, P, G> Region for FnRegion<F, O, P, G>
where
    O: Copy,
    P: Point,
    F: Fn(O) -> G + Send + Sync,
    G: Fn(P) -> P + Sync,
{
    type Pos = P;
    type Outer = O;
    type Prepared<'a>
        = G
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: O) -> G {
        (self.f)(outer)
    }
}

/// A prepared closure rule.
pub struct FnPrepared<G>(G);

impl<F, O, P, G> Rule<P, O> for FnRule<F, O, P, G>
where
    O: Copy,
    P: Point,
    F: Fn(O) -> G + Send + Sync,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Sync,
{
    type Prepared<'a>
        = FnPrepared<G>
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: O) -> FnPrepared<G> {
        FnPrepared((self.f)(outer))
    }
}

impl<P, G> PreparedRule<P> for FnPrepared<G>
where
    P: Point,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Sync,
{
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        (self.0)(best, rng, p)
    }
}

impl<F, O, I> Seeder for FnSeeder<F, O, I>
where
    O: Copy,
    I: Point,
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>> + Send + Sync,
{
    type Outer = O;
    type Inner = I;

    fn seed(&self, outer: O, rng: &mut dyn RngCore) -> Vec<Particle<I>> {
        (self.f)(outer, rng)
    }
}
