//! The traits every component implements, and the components users build
//! from closures.
//!
//! Every component is shown the position of the particle its search runs for
//! (`()` for a search that isn't nested) before that search begins. A search
//! runs on a fresh copy of its components, so what a component prepares for
//! one outer position never reaches another.

use crate::api::{BestSoFar, Particle, ParticleView, Point};
use rand::RngCore;
use std::marker::PhantomData;

/// A component that is shown the outer position before its search begins.
/// Components that don't need it are generic over `Outer` and ignore it.
pub trait OuterAware {
    type Outer: Copy + Send + Sync;

    fn see_outer(&mut self, outer: Self::Outer);
}

/// What a search maximizes.
pub trait Objective: OuterAware + Clone + Send + Sync {
    type Pos: Point;

    fn score(&self, pos: Self::Pos) -> f64;
}

/// Maps a position back into the feasible region.
pub trait Region: OuterAware + Clone + Send + Sync {
    type Pos: Point;

    fn confine(&self, pos: Self::Pos) -> Self::Pos;
}

/// Moves one particle, given the swarm's best so far. Takes any randomness
/// from `rng`, which belongs to the particle being moved.
pub trait Rule: OuterAware + Clone + Send + Sync {
    type Pos: Point;

    fn apply(&self, best: &BestSoFar<Self::Pos>, rng: &mut dyn RngCore, p: ParticleView<'_, Self::Pos>);
}

/// Creates the particles of an inner search from the outer position.
pub trait Seeder: Clone + Send + Sync {
    type Outer: Point;
    type Inner: Point;

    fn seed(&self, outer: Self::Outer, rng: &mut dyn RngCore) -> Vec<Particle<Self::Inner>>;
}

/// What objectives, regions and rules built from closures share. `make` is called with the outer position
/// once per search; what it returns serves that search.
pub struct FromFn<F, G, O, P> {
    make: F,
    ready: Option<G>,
    phantom: PhantomData<fn(O) -> P>,
}

// A copy starts unprepared: every search shows its components the outer
// position before it begins, so the copy is prepared for its own search.
impl<F: Clone, G, O, P> Clone for FromFn<F, G, O, P> {
    fn clone(&self) -> Self {
        FromFn {
            make: self.make.clone(),
            ready: None,
            phantom: PhantomData,
        }
    }
}

impl<F, G, O, P> FromFn<F, G, O, P> {
    fn ready(&self) -> &G {
        self.ready
            .as_ref()
            .expect("a component was used before its search showed it the outer position")
    }
}

impl<F, G, O, P> OuterAware for FromFn<F, G, O, P>
where
    F: Fn(O) -> G,
    O: Copy + Send + Sync,
{
    type Outer = O;

    fn see_outer(&mut self, outer: O) {
        self.ready = Some((self.make)(outer));
    }
}

/// An objective: `objective(|outer| move |pos| score)`.
pub fn objective<F, G, O, P>(make: F) -> ObjectiveFn<F, G, O, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> f64 + Send + Sync,
    O: Copy + Send + Sync,
    P: Point,
{
    ObjectiveFn(FromFn {
        make,
        ready: None,
        phantom: PhantomData,
    })
}

/// An objective built from a closure.
pub struct ObjectiveFn<F, G, O, P>(FromFn<F, G, O, P>);

impl<F: Clone, G, O, P> Clone for ObjectiveFn<F, G, O, P> {
    fn clone(&self) -> Self {
        ObjectiveFn(self.0.clone())
    }
}

impl<F, G, O, P> OuterAware for ObjectiveFn<F, G, O, P>
where
    F: Fn(O) -> G,
    O: Copy + Send + Sync,
{
    type Outer = O;

    fn see_outer(&mut self, outer: O) {
        self.0.see_outer(outer);
    }
}

impl<F, G, O, P> Objective for ObjectiveFn<F, G, O, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> f64 + Send + Sync,
    O: Copy + Send + Sync,
    P: Point,
{
    type Pos = P;

    fn score(&self, pos: P) -> f64 {
        (self.0.ready())(pos)
    }
}

/// A region: `region(|outer| move |pos| confined_pos)`.
pub fn region<F, G, O, P>(make: F) -> RegionFn<F, G, O, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> P + Send + Sync,
    O: Copy + Send + Sync,
    P: Point,
{
    RegionFn(FromFn {
        make,
        ready: None,
        phantom: PhantomData,
    })
}

/// A region built from a closure.
pub struct RegionFn<F, G, O, P>(FromFn<F, G, O, P>);

impl<F: Clone, G, O, P> Clone for RegionFn<F, G, O, P> {
    fn clone(&self) -> Self {
        RegionFn(self.0.clone())
    }
}

impl<F, G, O, P> OuterAware for RegionFn<F, G, O, P>
where
    F: Fn(O) -> G,
    O: Copy + Send + Sync,
{
    type Outer = O;

    fn see_outer(&mut self, outer: O) {
        self.0.see_outer(outer);
    }
}

impl<F, G, O, P> Region for RegionFn<F, G, O, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> P + Send + Sync,
    O: Copy + Send + Sync,
    P: Point,
{
    type Pos = P;

    fn confine(&self, pos: P) -> P {
        (self.0.ready())(pos)
    }
}

/// A rule: `rule(|outer| move |best, rng, particle| { ... })`.
pub fn rule<F, G, O, P>(make: F) -> RuleFn<F, G, O, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Send + Sync,
    O: Copy + Send + Sync,
    P: Point,
{
    RuleFn(FromFn {
        make,
        ready: None,
        phantom: PhantomData,
    })
}

/// A rule built from a closure.
pub struct RuleFn<F, G, O, P>(FromFn<F, G, O, P>);

impl<F: Clone, G, O, P> Clone for RuleFn<F, G, O, P> {
    fn clone(&self) -> Self {
        RuleFn(self.0.clone())
    }
}

impl<F, G, O, P> OuterAware for RuleFn<F, G, O, P>
where
    F: Fn(O) -> G,
    O: Copy + Send + Sync,
{
    type Outer = O;

    fn see_outer(&mut self, outer: O) {
        self.0.see_outer(outer);
    }
}

impl<F, G, O, P> Rule for RuleFn<F, G, O, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Send + Sync,
    O: Copy + Send + Sync,
    P: Point,
{
    type Pos = P;

    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        (self.0.ready())(best, rng, p)
    }
}

/// A seeder: `seeder(|outer, rng| particles)`.
pub fn seeder<F, O, I>(make: F) -> SeederFn<F, O, I>
where
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>> + Clone + Send + Sync,
    O: Point,
    I: Point,
{
    SeederFn {
        make,
        phantom: PhantomData,
    }
}

pub struct SeederFn<F, O, I> {
    make: F,
    phantom: PhantomData<fn(O) -> I>,
}

impl<F: Clone, O, I> Clone for SeederFn<F, O, I> {
    fn clone(&self) -> Self {
        SeederFn {
            make: self.make.clone(),
            phantom: PhantomData,
        }
    }
}

impl<F, O, I> Seeder for SeederFn<F, O, I>
where
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>> + Clone + Send + Sync,
    O: Point,
    I: Point,
{
    type Outer = O;
    type Inner = I;

    fn seed(&self, outer: O, rng: &mut dyn RngCore) -> Vec<Particle<I>> {
        (self.make)(outer, rng)
    }
}
