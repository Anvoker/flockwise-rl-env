//! The two-part problem used by several tests.
//!
//! Fitness: -|x - a|^2 - |t - (x + c)|^2, with t constrained to [x - w, x + w]
//! and x constrained to a box. Optimum: x = a, t = a + clamp(c, -w, w).
#![allow(dead_code)]

use super::*;
use flockwise::{Region, OuterAware, Objective, Seeder, Particle};
use rand::Rng;
use rand::rngs::StdRng;

pub const K: usize = 3;

/// Where outer particles start. The box itself is much wider, so that
/// particles rarely reach its faces: PSO particles clamped onto a face can get
/// stuck there.
pub const INIT_LO: [f64; K] = [-6.0; K];
pub const INIT_HI: [f64; K] = [6.0; K];

vector_unit!(X, K);
vector_unit!(T, K);
pair_unit!(W, x: X, t: T);

#[derive(Copy, Clone, Debug)]
pub struct Problem {
    pub a: [f64; K],
    pub c: [f64; K],
    pub w: f64,
    pub x_lo: [f64; K],
    pub x_hi: [f64; K],
}

impl Problem {
    pub fn fitness(&self, x: [f64; K], t: [f64; K]) -> f64 {
        let target: [f64; K] = std::array::from_fn(|i| x[i] + self.c[i]);
        -sq_dist(x, self.a) - sq_dist(t, target)
    }

    pub fn optimum(&self) -> ([f64; K], [f64; K]) {
        let t = std::array::from_fn(|i| self.a[i] + self.c[i].clamp(-self.w, self.w));
        (self.a, t)
    }
}

#[derive(Clone, Debug)]
pub struct OuterFit(pub Problem);

impl OuterAware for OuterFit {
    type Outer = ();
    fn see_outer(&mut self, _: ()) {}
}

impl Objective for OuterFit {
    type Pos = W;
    fn score(&self, p: W) -> f64 {
        self.0.fitness(p.x.0, p.t.0)
    }
}

#[derive(Clone, Debug)]
pub struct XBounds(pub Problem);

impl OuterAware for XBounds {
    type Outer = ();
    fn see_outer(&mut self, _: ()) {}
}

impl Region for XBounds {
    type Pos = X;
    fn confine(&self, x: X) -> X {
        X(clamp(x.0, self.0.x_lo, self.0.x_hi))
    }
}

/// Scores `t` for the outer particle's `x`.
#[derive(Clone, Debug)]
pub struct InnerFit {
    pub problem: Problem,
    pub outer: W,
}

impl OuterAware for InnerFit {
    type Outer = W;
    fn see_outer(&mut self, outer: W) {
        self.outer = outer;
    }
}

impl Objective for InnerFit {
    type Pos = T;
    fn score(&self, t: T) -> f64 {
        self.problem.fitness(self.outer.x.0, t.0)
    }
}

/// Keeps `t` within `w` of the outer particle's `x`.
#[derive(Clone, Debug)]
pub struct TBounds {
    pub problem: Problem,
    pub outer: W,
}

impl OuterAware for TBounds {
    type Outer = W;
    fn see_outer(&mut self, outer: W) {
        self.outer = outer;
    }
}

impl Region for TBounds {
    type Pos = T;
    fn confine(&self, t: T) -> T {
        let x = self.outer.x.0;
        T(clamp(t.0, add(x, -self.problem.w), add(x, self.problem.w)))
    }
}

#[derive(Clone, Debug)]
pub struct TInit {
    pub problem: Problem,
    pub count: usize,
}

impl Seeder for TInit {
    type Outer = W;
    type Inner = T;

    fn seed<R: Rng + ?Sized>(&self, outer: W, rng: &mut R) -> Vec<Particle<T>> {
        let x = outer.x.0;
        let w = self.problem.w;
        (0..self.count)
            .map(|_| {
                let t = T(uniform(rng, add(x, -w), add(x, w)));
                let v = T(uniform(rng, [-w; K], [w; K])) * 0.1;
                Particle::new(t, v)
            })
            .collect()
    }
}

pub fn initial_group(problem: &Problem, count: usize, rng: &mut StdRng) -> Vec<Particle<W>> {
    (0..count)
        .map(|_| {
            let x = X(clamp(uniform(rng, INIT_LO, INIT_HI), problem.x_lo, problem.x_hi));
            let v = X(uniform(rng, [-1.0; K], [1.0; K]));
            Particle::new(W { x, t: T(x.0) }, W { x: v, t: T::default() })
        })
        .collect()
}


impl Problem {
    pub fn standard() -> Problem {
        Problem {
            a: [3.0, -2.0, 4.0],
            c: [0.5, 2.5, -3.0],
            w: 1.0,
            x_lo: [-30.0; K],
            x_hi: [30.0; K],
        }
    }
}

impl InnerFit {
    pub fn new(problem: Problem) -> Self {
        InnerFit { problem, outer: W::default() }
    }
}

impl TBounds {
    pub fn new(problem: Problem) -> Self {
        TBounds { problem, outer: W::default() }
    }
}
