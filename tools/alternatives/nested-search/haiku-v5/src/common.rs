//! The parts a search is built from, and how a search runs them.
//!
//! A search prepares each of its parts once, when it starts, with the outer
//! position it runs for. Each iteration then scores the particles, updates the
//! bests, and runs the rules, in parallel over the particles. Random numbers
//! come from streams derived from the seed, one per part and particle, so the
//! results don't depend on the thread count or on what other parts draw.

use crate::api::{BestSoFar, Particle, ParticleView, Point, ReadFrom, SplitFrom};
use crate::combine::ForField;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rayon::prelude::*;
use std::marker::PhantomData;

/// Scores positions. Prepared once per search.
pub trait Objective: Send + Sync {
    type Pos: Point;
    type Outer: Copy;
    type Prepared: Fn(Self::Pos) -> f64 + Send + Sync;

    fn prepare(&self, outer: Self::Outer) -> Self::Prepared;
}

/// Maps a position back into the feasible region. Prepared once per search.
pub trait Region: Send + Sync {
    type Pos: Point;
    type Outer: Copy;
    type Prepared: Fn(Self::Pos) -> Self::Pos + Send + Sync;

    fn prepare(&self, outer: Self::Outer) -> Self::Prepared;
}

/// Makes the particles an inner search starts from.
pub trait Seeder: Send + Sync {
    type Outer: Copy;
    type Inner: Point;

    fn seed(&self, outer: Self::Outer, rng: &mut dyn RngCore) -> Vec<Particle<Self::Inner>>;
}

/// Moves particles. Prepared once per search.
pub trait Rule: Send + Sync {
    type Pos: Point;
    type Outer: Copy;
    type Prepared: Send + Sync;

    fn prepare(&self, outer: Self::Outer) -> Self::Prepared;

    /// Moves one particle, given the best position the search has found so far.
    fn apply(
        &self,
        prepared: &Self::Prepared,
        best: &BestSoFar<Self::Pos>,
        rng: &mut dyn RngCore,
        p: ParticleView<'_, Self::Pos>,
    );

    /// Runs this rule on one field of the whole position.
    fn for_field<F>(self) -> ForField<Self, F>
    where
        Self: Sized + Rule<Pos = F>,
        F: SplitFrom + ReadFrom<Whole = <F as SplitFrom>::Whole>,
    {
        ForField::new(self)
    }
}

/// A search over positions of type `Pos`, for outer positions of type
/// `Outer`. A search that isn't nested has `Outer = ()`.
pub trait Search: Send + Sync {
    type Pos: Point;
    type Outer: Copy;

    #[doc(hidden)]
    type Prepared: Send + Sync;

    #[doc(hidden)]
    fn prepare(&self, outer: Self::Outer) -> Self::Prepared;

    /// One iteration: score the particles, update the bests, then run the rules.
    #[doc(hidden)]
    fn step(
        &self,
        prepared: &Self::Prepared,
        key: u64,
        best: &mut BestSoFar<Self::Pos>,
        particles: &mut [Particle<Self::Pos>],
    );
}

// Closure wrappers. Each keeps the closure that takes the outer position, and
// calls it once per search.

#[derive(Clone)]
pub struct ObjectiveFn<F, O, P> {
    f: F,
    marker: PhantomData<fn(O) -> P>,
}

#[derive(Clone)]
pub struct RegionFn<F, O, P> {
    f: F,
    marker: PhantomData<fn(O) -> P>,
}

#[derive(Clone)]
pub struct RuleFn<F, O, P> {
    f: F,
    marker: PhantomData<fn(O) -> P>,
}

#[derive(Clone)]
pub struct SeederFn<F, O, I> {
    f: F,
    marker: PhantomData<fn(O) -> I>,
}

/// A scoring function, from the outer position: `f(outer)` returns the function
/// that scores positions in a search.
pub fn objective<O, P, F, G>(f: F) -> ObjectiveFn<F, O, P>
where
    F: Fn(O) -> G,
    G: Fn(P) -> f64,
{
    ObjectiveFn { f, marker: PhantomData }
}

/// A region, from the outer position: `f(outer)` returns the function that maps
/// positions back into the feasible region.
pub fn region<O, P, F, G>(f: F) -> RegionFn<F, O, P>
where
    F: Fn(O) -> G,
    G: Fn(P) -> P,
{
    RegionFn { f, marker: PhantomData }
}

/// A rule, from the outer position: `f(outer)` returns the function that moves
/// one particle.
pub fn rule<O, P, F, G>(f: F) -> RuleFn<F, O, P>
where
    F: Fn(O) -> G,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>),
{
    RuleFn { f, marker: PhantomData }
}

/// Starting particles for an inner search: `f(outer, rng)`.
pub fn seeder<O, I, F>(f: F) -> SeederFn<F, O, I>
where
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>>,
{
    SeederFn { f, marker: PhantomData }
}

impl<F, G, O, P> Objective for ObjectiveFn<F, O, P>
where
    F: Fn(O) -> G + Send + Sync,
    G: Fn(P) -> f64 + Send + Sync,
    O: Copy,
    P: Point,
{
    type Pos = P;
    type Outer = O;
    type Prepared = G;

    fn prepare(&self, outer: O) -> G {
        (self.f)(outer)
    }
}

impl<F, G, O, P> Region for RegionFn<F, O, P>
where
    F: Fn(O) -> G + Send + Sync,
    G: Fn(P) -> P + Send + Sync,
    O: Copy,
    P: Point,
{
    type Pos = P;
    type Outer = O;
    type Prepared = G;

    fn prepare(&self, outer: O) -> G {
        (self.f)(outer)
    }
}

impl<F, G, O, P> Rule for RuleFn<F, O, P>
where
    F: Fn(O) -> G + Send + Sync,
    G: Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Send + Sync,
    O: Copy,
    P: Point,
{
    type Pos = P;
    type Outer = O;
    type Prepared = G;

    fn prepare(&self, outer: O) -> G {
        (self.f)(outer)
    }

    fn apply(&self, prepared: &G, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        prepared(best, rng, p)
    }
}

impl<F, I, O> Seeder for SeederFn<F, O, I>
where
    F: Fn(O, &mut dyn RngCore) -> Vec<Particle<I>> + Send + Sync,
    O: Copy,
    I: Point,
{
    type Outer = O;
    type Inner = I;

    fn seed(&self, outer: O, rng: &mut dyn RngCore) -> Vec<Particle<I>> {
        (self.f)(outer, rng)
    }
}

// Random streams. A stream is named by a key and a slot; every name gives an
// unrelated stream, so parts never share numbers.

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The key of part `slot` of the stream `key`.
pub(crate) fn child(key: u64, slot: u64) -> u64 {
    splitmix64(splitmix64(key) ^ slot)
}

/// The generator for part `slot` of the stream `key`.
pub(crate) fn stream(key: u64, slot: u64) -> StdRng {
    StdRng::seed_from_u64(child(key, slot))
}

/// Forgets what particles found earlier: their bests and scores. Positions and
/// velocities stay.
pub(crate) fn reset<P: Point>(particles: &mut [Particle<P>]) {
    for p in particles.iter_mut() {
        p.score = f64::NEG_INFINITY;
        p.best_pos = p.pos;
        p.best_score = f64::NEG_INFINITY;
    }
}

/// Scores every particle at its current position, then updates the bests in
/// slice order, so that on a tie the earlier particle wins.
pub(crate) fn score_particles<P, S>(score: &S, best: &mut BestSoFar<P>, particles: &mut [Particle<P>])
where
    P: Point,
    S: Fn(P) -> f64 + Sync,
{
    particles.par_iter_mut().for_each(|p| p.score = score(p.pos));

    for p in particles.iter_mut() {
        if p.score > p.best_score {
            p.best_pos = p.pos;
            p.best_score = p.score;

            if p.score > best.best_score {
                best.best_pos = p.pos;
                best.best_score = p.score;
            }
        }
    }
}

/// Runs a search from fresh state for `iterations` iterations, and returns the
/// best it found.
pub(crate) fn run_search<S: Search>(
    search: &S,
    outer: S::Outer,
    iterations: usize,
    particles: &mut [Particle<S::Pos>],
    seed: u64,
) -> BestSoFar<S::Pos> {
    let prepared = search.prepare(outer);
    reset(particles);
    let mut best = BestSoFar::new();
    for k in 0..iterations {
        search.step(&prepared, child(seed, k as u64), &mut best, particles);
    }
    best
}

/// Runs a search that isn't nested, one iteration at a time. Yields the best
/// so far after each iteration. Afterwards `particles` holds the particles'
/// final state.
pub struct Run<'a, S: Search> {
    search: &'a mut S,
    prepared: S::Prepared,
    best: BestSoFar<S::Pos>,
    particles: &'a mut [Particle<S::Pos>],
    seed: u64,
    iterations: usize,
    iteration: usize,
}

impl<'a, S: Search<Outer = ()>> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut [Particle<S::Pos>], seed: u64) -> Self {
        reset(particles);
        let prepared = search.prepare(());
        Run {
            search,
            prepared,
            best: BestSoFar::new(),
            particles,
            seed,
            iterations,
            iteration: 0,
        }
    }
}

impl<S: Search<Outer = ()>> Iterator for Run<'_, S> {
    type Item = BestSoFar<S::Pos>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iteration >= self.iterations {
            return None;
        }
        let key = child(self.seed, self.iteration as u64);
        self.search.step(&self.prepared, key, &mut self.best, &mut *self.particles);
        self.iteration += 1;
        Some(self.best)
    }
}
