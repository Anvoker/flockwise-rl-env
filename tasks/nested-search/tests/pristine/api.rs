//! Public traits and types that users implement or construct.
//!
//! This file is part of the public contract and must not be changed.

use rand::Rng;
use std::fmt::Debug;
use std::ops::{Add, Mul, Sub};

/// A position in a search space.
pub trait Point:
    Copy
    + Default
    + Debug
    + Send
    + Sync
    + 'static
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<f64, Output = Self>
{
    /// Applies `f` to every scalar coordinate, always in the same order.
    fn map_scalars(self, f: impl FnMut(f64) -> f64) -> Self;
}

/// A component that can be shown the position of the particle in the
/// enclosing search.
///
/// Every objective, region and rule is `OuterAware`. Components that don't
/// need the outer position declare some `Outer` type and ignore it.
pub trait OuterAware {
    type Outer: Copy + Send + Sync;

    fn see_outer(&mut self, outer: Self::Outer);
}

/// What a search maximizes. Particles may be scored from several threads at once.
pub trait Objective: OuterAware + Send + Sync {
    type Pos: Point;

    fn score(&self, pos: Self::Pos) -> f64;
}

/// Maps a position back into the feasible region. May be used from several
/// threads at once.
pub trait Region: OuterAware + Send + Sync {
    type Pos: Point;

    fn confine(&self, pos: Self::Pos) -> Self::Pos;
}

/// Splits a view of a whole particle into a view of one of its fields.
pub trait SplitFrom: Point {
    type Whole: Point;

    fn split(whole: ParticleView<'_, Self::Whole>) -> ParticleView<'_, Self>;
}

/// Reads one field out of a whole position.
pub trait ReadFrom: Point {
    type Whole: Point;

    fn read(whole: &Self::Whole) -> Self;
}

/// Writes one field into a whole position.
pub trait WriteInto: Point {
    type Whole: Point;

    fn write(self, whole: &mut Self::Whole);
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Particle<P> {
    pub pos: P,
    pub vel: P,
    pub score: f64,
    pub best_pos: P,
    pub best_score: f64,
}

impl<P: Copy> Particle<P> {
    /// A particle that hasn't been scored yet.
    pub fn new(pos: P, vel: P) -> Self {
        Particle {
            pos,
            vel,
            score: f64::NEG_INFINITY,
            best_pos: pos,
            best_score: f64::NEG_INFINITY,
        }
    }

    pub fn view(&mut self) -> ParticleView<'_, P> {
        ParticleView {
            pos: &mut self.pos,
            vel: &mut self.vel,
            score: &mut self.score,
            best_pos: &mut self.best_pos,
            best_score: &mut self.best_score,
        }
    }
}

/// A mutable view of a particle, or of one field of a particle.
pub struct ParticleView<'a, P> {
    pub pos: &'a mut P,
    pub vel: &'a mut P,
    pub score: &'a mut f64,
    pub best_pos: &'a mut P,
    pub best_score: &'a mut f64,
}

impl<P> ParticleView<'_, P> {
    pub fn reborrow(&mut self) -> ParticleView<'_, P> {
        ParticleView {
            pos: self.pos,
            vel: self.vel,
            score: self.score,
            best_pos: self.best_pos,
            best_score: self.best_score,
        }
    }
}

/// The best position a whole swarm has found.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BestSoFar<P> {
    pub best_pos: P,
    pub best_score: f64,
}

impl<P: Default> BestSoFar<P> {
    pub fn new() -> Self {
        BestSoFar {
            best_pos: P::default(),
            best_score: f64::NEG_INFINITY,
        }
    }
}

impl<P: Default> Default for BestSoFar<P> {
    fn default() -> Self {
        BestSoFar::new()
    }
}

/// Moves one particle. `Shared` is what the search shares with every
/// particle, e.g. the best position so far.
///
/// Particles may be moved from several threads at once, so a rule takes any
/// randomness it needs from `rng`, which belongs to the particle being moved.
pub trait Rule: OuterAware + Send + Sync {
    type Pos: Point;
    type Shared;

    fn apply<R: Rng + ?Sized>(&self, shared: &Self::Shared, rng: &mut R, p: ParticleView<'_, Self::Pos>);
}

/// Creates the particles of an inner search from the outer position it runs for.
pub trait Seeder: Send + Sync {
    type Outer: Point;
    type Inner: Point;

    fn seed<R: Rng + ?Sized>(&self, outer: Self::Outer, rng: &mut R) -> Vec<Particle<Self::Inner>>;
}
