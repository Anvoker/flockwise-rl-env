// Shared by every compile check. Written against the public API only.
#![allow(dead_code, unused_imports)]

use flockwise::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::marker::PhantomData;
use std::ops::{Add, Mul, Sub};

macro_rules! unit {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, Default, PartialEq)]
        struct $name([f64; 2]);
        impl Add for $name {
            type Output = Self;
            fn add(self, r: Self) -> Self { $name([self.0[0] + r.0[0], self.0[1] + r.0[1]]) }
        }
        impl Sub for $name {
            type Output = Self;
            fn sub(self, r: Self) -> Self { $name([self.0[0] - r.0[0], self.0[1] - r.0[1]]) }
        }
        impl Mul<f64> for $name {
            type Output = Self;
            fn mul(self, r: f64) -> Self { $name([self.0[0] * r, self.0[1] * r]) }
        }
        impl Point for $name {
            fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self { $name(self.0.map(|v| f(v))) }
        }
    };
}

unit!(X);
unit!(T);
unit!(V);

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct W {
    x: X,
    t: T,
}
impl Add for W {
    type Output = Self;
    fn add(self, r: Self) -> Self { W { x: self.x + r.x, t: self.t + r.t } }
}
impl Sub for W {
    type Output = Self;
    fn sub(self, r: Self) -> Self { W { x: self.x - r.x, t: self.t - r.t } }
}
impl Mul<f64> for W {
    type Output = Self;
    fn mul(self, r: f64) -> Self { W { x: self.x * r, t: self.t * r } }
}
impl Point for W {
    fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
        W { x: self.x.map_scalars(&mut f), t: self.t.map_scalars(&mut f) }
    }
}
macro_rules! field_of {
    ($f:ident : $ty:ty) => {
        impl SplitFrom for $ty {
            type Whole = W;
            fn split(w: ParticleView<'_, W>) -> ParticleView<'_, Self> {
                ParticleView { pos: &mut w.pos.$f, vel: &mut w.vel.$f, score: w.score, best_pos: &mut w.best_pos.$f, best_score: w.best_score }
            }
        }
        impl ReadFrom for $ty {
            type Whole = W;
            fn read(w: &W) -> Self { w.$f }
        }
        impl WriteInto for $ty {
            type Whole = W;
            fn write(self, w: &mut W) { w.$f = self; }
        }
    };
}
field_of!(x: X);
field_of!(t: T);

/// A fitness over unit `U` that takes context `C`.
#[derive(Clone, Debug)]
struct FitOn<U, C>(PhantomData<fn() -> (U, C)>);
impl<U, C> FitOn<U, C> {
    fn new() -> Self { FitOn(PhantomData) }
}
impl<U, C: Copy + Send + Sync> OuterAware for FitOn<U, C> {
    type Outer = C;
    fn see_outer(&mut self, _: C) {}
}
impl<U: Point, C: Copy + Send + Sync> Objective for FitOn<U, C> {
    type Pos = U;
    fn score(&self, _: U) -> f64 { 0.0 }
}

/// A boundary over unit `U` that takes context `C`.
#[derive(Clone, Debug)]
struct BoundOn<U, C>(PhantomData<fn() -> (U, C)>);
impl<U, C> BoundOn<U, C> {
    fn new() -> Self { BoundOn(PhantomData) }
}
impl<U, C: Copy + Send + Sync> OuterAware for BoundOn<U, C> {
    type Outer = C;
    fn see_outer(&mut self, _: C) {}
}
impl<U: Point, C: Copy + Send + Sync> Region for BoundOn<U, C> {
    type Pos = U;
    fn confine(&self, u: U) -> U { u }
}

/// Initializes inner particles of unit `I` from an outer particle of unit `O`.
#[derive(Clone, Debug)]
struct InitFor<O, I>(PhantomData<fn() -> (O, I)>);
impl<O, I> InitFor<O, I> {
    fn new() -> Self { InitFor(PhantomData) }
}
impl<O: Point, I: Point> Seeder for InitFor<O, I> {
    type Outer = O;
    type Inner = I;
    fn seed<R: Rng + ?Sized>(&self, _: O, _: &mut R) -> Vec<Particle<I>> { Vec::new() }
}

fn cfg() -> PsoParams {
    PsoParams { inertia: 0.7, cognitive: 1.5, social: 1.5 }
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(0)
}
