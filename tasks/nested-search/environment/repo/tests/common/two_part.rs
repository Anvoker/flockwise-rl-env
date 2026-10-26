//! The two-part problem used by several tests.
//!
//! Fitness: -|x - a|^2 - |t - (x + c)|^2, with t constrained to [x - w, x + w]
//! and x constrained to a box. Optimum: x = a, t = a + clamp(c, -w, w).
#![allow(dead_code)]

use super::*;
use flockwise::Particle;
use rand::RngCore;
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

// Components are built from closures. Each function below returns the
// closure a component uses during one search; the call sites wrap them, as in
// `objective(move |w| inner_fit(problem, w))`.

pub fn outer_fit(problem: Problem) -> impl Fn(W) -> f64 + Clone + Send + Sync {
    move |p: W| problem.fitness(p.x.0, p.t.0)
}

pub fn x_bounds(problem: Problem) -> impl Fn(X) -> X + Clone + Send + Sync {
    move |x: X| X(clamp(x.0, problem.x_lo, problem.x_hi))
}

/// Scores `t` for the outer particle's `x`.
pub fn inner_fit(problem: Problem, outer: W) -> impl Fn(T) -> f64 + Clone + Send + Sync {
    move |t: T| problem.fitness(outer.x.0, t.0)
}

/// Keeps `t` within `w` of the outer particle's `x`.
pub fn t_bounds(problem: Problem, outer: W) -> impl Fn(T) -> T + Clone + Send + Sync {
    let (lo, hi) = (add(outer.x.0, -problem.w), add(outer.x.0, problem.w));
    move |t: T| T(clamp(t.0, lo, hi))
}

/// Starting particles for an inner search over `t`.
pub fn t_init(problem: Problem, count: usize) -> impl Fn(W, &mut dyn RngCore) -> Vec<Particle<T>> + Clone + Send + Sync {
    move |outer: W, rng: &mut dyn RngCore| {
        let x = outer.x.0;
        let w = problem.w;
        (0..count)
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

