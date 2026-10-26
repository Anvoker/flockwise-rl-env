// Shared by every compile check. Written against the public API only.
#![allow(dead_code, unused_imports)]

use nestswarm::*;
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
        impl Unit for $name {
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
impl Unit for W {
    fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
        W { x: self.x.map_scalars(&mut f), t: self.t.map_scalars(&mut f) }
    }
}
impl PartOf<W> for X {
    fn get(w: &W) -> Self { w.x }
    fn get_mut(w: &mut W) -> &mut Self { &mut w.x }
}
impl PartOf<W> for T {
    fn get(w: &W) -> Self { w.t }
    fn get_mut(w: &mut W) -> &mut Self { &mut w.t }
}

/// A fitness over unit `U` that takes context `C`.
#[derive(Clone, Debug)]
struct FitOn<U, C>(PhantomData<fn() -> (U, C)>);
impl<U, C> FitOn<U, C> {
    fn new() -> Self { FitOn(PhantomData) }
}
impl<U, C: Copy> Contextful for FitOn<U, C> {
    type TContext = C;
    fn set_context(&mut self, _: C) {}
}
impl<U: Unit, C: Copy> FitCalc for FitOn<U, C> {
    type T = U;
    fn calculate_fit(&self, _: U) -> f64 { 0.0 }
}

/// A boundary over unit `U` that takes context `C`.
#[derive(Clone, Debug)]
struct BoundOn<U, C>(PhantomData<fn() -> (U, C)>);
impl<U, C> BoundOn<U, C> {
    fn new() -> Self { BoundOn(PhantomData) }
}
impl<U, C: Copy> Contextful for BoundOn<U, C> {
    type TContext = C;
    fn set_context(&mut self, _: C) {}
}
impl<U: Unit, C: Copy> Boundary for BoundOn<U, C> {
    type T = U;
    fn handle(&self, u: U) -> U { u }
}

/// Initializes inner particles of unit `I` from an outer particle of unit `O`.
#[derive(Clone, Debug)]
struct InitFor<O, I>(PhantomData<fn() -> (O, I)>);
impl<O, I> InitFor<O, I> {
    fn new() -> Self { InitFor(PhantomData) }
}
impl<O: Unit, I: Unit> NestedInit for InitFor<O, I> {
    type TOuter = O;
    type TInner = I;
    fn init<R: Rng + ?Sized>(&self, _: O, _: &mut R) -> Vec<Particle<I>> { Vec::new() }
}

fn cfg() -> SearchConfigGBest {
    SearchConfigGBest { inertia: 0.7, cognitive_coeff: 1.5, social_coeff: 1.5 }
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(0)
}
