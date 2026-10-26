//! Public traits and types that users implement or construct.
//!
//! This file is part of the public contract and must not be changed.

use rand::Rng;
use std::fmt::Debug;
use std::ops::{Add, Mul, Sub};

/// A position in a search space.
pub trait Unit:
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

/// A component that can be told the position of the particle in the enclosing search.
///
/// Every fitness function, boundary and mover is `Contextful`. Components that
/// don't need context declare some `TContext` and ignore it in `set_context`.
pub trait Contextful {
    type TContext: Copy;

    fn set_context(&mut self, context: Self::TContext);
}

/// A fitness function. Higher is better.
pub trait FitCalc: Contextful {
    type T: Unit;

    fn calculate_fit(&self, position: Self::T) -> f64;
}

/// Maps a position back into the feasible region.
pub trait Boundary: Contextful {
    type T: Unit;

    fn handle(&self, position: Self::T) -> Self::T;
}

/// Declares that `Self` is stored as a part of the whole unit `W`.
pub trait PartOf<W: Unit>: Unit {
    fn get(whole: &W) -> Self;
    fn get_mut(whole: &mut W) -> &mut Self;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Particle<T> {
    pub pos: T,
    pub vel: T,
    pub fit: f64,
    pub best_pos: T,
    pub best_fit: f64,
}

impl<T: Copy> Particle<T> {
    /// A particle that hasn't been evaluated yet.
    pub fn new(pos: T, vel: T) -> Self {
        Particle {
            pos,
            vel,
            fit: f64::NEG_INFINITY,
            best_pos: pos,
            best_fit: f64::NEG_INFINITY,
        }
    }

    pub fn as_mut(&mut self) -> ParticleMut<'_, T> {
        ParticleMut {
            pos: &mut self.pos,
            vel: &mut self.vel,
            fit: &mut self.fit,
            best_pos: &mut self.best_pos,
            best_fit: &mut self.best_fit,
        }
    }
}

/// A mutable view of a particle, or of one part of a particle.
pub struct ParticleMut<'a, T> {
    pub pos: &'a mut T,
    pub vel: &'a mut T,
    pub fit: &'a mut f64,
    pub best_pos: &'a mut T,
    pub best_fit: &'a mut f64,
}

impl<T> ParticleMut<'_, T> {
    pub fn reborrow(&mut self) -> ParticleMut<'_, T> {
        ParticleMut {
            pos: self.pos,
            vel: self.vel,
            fit: self.fit,
            best_pos: self.best_pos,
            best_fit: self.best_fit,
        }
    }
}

/// The best position found by a whole swarm.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GBest<T> {
    pub best_pos: T,
    pub best_fit: f64,
}

impl<T: Default> GBest<T> {
    pub fn new() -> Self {
        GBest {
            best_pos: T::default(),
            best_fit: f64::NEG_INFINITY,
        }
    }
}

impl<T: Default> Default for GBest<T> {
    fn default() -> Self {
        GBest::new()
    }
}

/// Moves one particle. `TCommon` is what the search shares with every
/// particle, e.g. the global best.
pub trait Mover: Contextful {
    type TUnit: Unit;
    type TCommon;

    fn update(&mut self, common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>);
}

/// Creates the particles of an inner search from the outer particle it runs for.
pub trait NestedInit {
    type TOuter: Unit;
    type TInner: Unit;

    fn init<R: Rng + ?Sized>(&self, outer: Self::TOuter, rng: &mut R)
    -> Vec<Particle<Self::TInner>>;
}
