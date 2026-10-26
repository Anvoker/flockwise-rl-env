//! An alternative design, used to check that the verifier accepts solutions
//! built differently from the reference.
//!
//! Every component is generic over the outer position it is prepared for
//! (`Rule<O>`), and preparing it for one search builds a separate value that
//! serves that search (`Rule::Ready`). Nothing is mutated or reset: a search
//! that runs again prepares new values.

use crate::api::{BestSoFar, Particle, ParticleView, Point};
use rand::RngCore;
use std::marker::PhantomData;

/// Scores positions during one search.
pub trait Score<P>: Send + Sync {
    fn score(&self, pos: P) -> f64;
}

impl<P, F: Fn(P) -> f64 + Send + Sync> Score<P> for F {
    fn score(&self, pos: P) -> f64 {
        self(pos)
    }
}

/// Confines positions during one search.
pub trait Confine<P>: Send + Sync {
    fn confine(&self, pos: P) -> P;
}

impl<P, F: Fn(P) -> P + Send + Sync> Confine<P> for F {
    fn confine(&self, pos: P) -> P {
        self(pos)
    }
}

/// Moves particles during one search.
pub trait Move<P>: Send + Sync {
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>);
}

impl<P, F> Move<P> for F
where
    F: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Send + Sync,
{
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        self(best, rng, p)
    }
}

pub trait Objective<O>: Clone + Send + Sync {
    type Pos: Point;
    type Ready: Score<Self::Pos>;

    fn prepare(&self, outer: O) -> Self::Ready;
}

pub trait Region<O>: Clone + Send + Sync {
    type Pos: Point;
    type Ready: Confine<Self::Pos>;

    fn prepare(&self, outer: O) -> Self::Ready;
}

pub trait Rule<O>: Clone + Send + Sync {
    type Pos: Point;
    type Ready: Move<Self::Pos>;

    fn prepare(&self, outer: O) -> Self::Ready;
}

pub trait Seeder: Clone + Send + Sync {
    type Outer: Point;
    type Inner: Point;

    fn seed(&self, outer: Self::Outer, rng: &mut dyn RngCore) -> Vec<Particle<Self::Inner>>;
}

#[derive(Clone)]
pub struct ObjectiveFn<F, P>(F, PhantomData<fn(P)>);

pub fn objective<F, P>(make: F) -> ObjectiveFn<F, P> {
    ObjectiveFn(make, PhantomData)
}

impl<F, G, O, P> Objective<O> for ObjectiveFn<F, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> f64 + Send + Sync,
    P: Point,
{
    type Pos = P;
    type Ready = G;

    fn prepare(&self, outer: O) -> G {
        (self.0)(outer)
    }
}

#[derive(Clone)]
pub struct RegionFn<F, P>(F, PhantomData<fn(P)>);

pub fn region<F, P>(make: F) -> RegionFn<F, P> {
    RegionFn(make, PhantomData)
}

impl<F, G, O, P> Region<O> for RegionFn<F, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(P) -> P + Send + Sync,
    P: Point,
{
    type Pos = P;
    type Ready = G;

    fn prepare(&self, outer: O) -> G {
        (self.0)(outer)
    }
}

#[derive(Clone)]
pub struct RuleFn<F, P>(F, PhantomData<fn(P)>);

pub fn rule<F, P>(make: F) -> RuleFn<F, P> {
    RuleFn(make, PhantomData)
}

impl<F, G, O, P> Rule<O> for RuleFn<F, P>
where
    F: Fn(O) -> G + Clone + Send + Sync,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Send + Sync,
    P: Point,
{
    type Pos = P;
    type Ready = G;

    fn prepare(&self, outer: O) -> G {
        (self.0)(outer)
    }
}

#[derive(Clone)]
pub struct SeederFn<F, O, I>(F, PhantomData<fn(O) -> I>);

pub fn seeder<F, O, I>(make: F) -> SeederFn<F, O, I>
where
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>> + Clone + Send + Sync,
{
    SeederFn(make, PhantomData)
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
        (self.0)(outer, rng)
    }
}
