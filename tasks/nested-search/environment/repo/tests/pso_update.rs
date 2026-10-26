//! One update of the PSO mover, checked against the formula.

mod common;

use common::*;
use flockwise::{BestSoFar, PsoRule, Rule, Particle, PsoParams};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

vector_unit!(V, 6);

const CFG: PsoParams = PsoParams {
    inertia: 0.5,
    cognitive: 1.5,
    social: 2.0,
};

fn pso(seed: u64) -> impl Rule<Pos = V, Shared = BestSoFar<V>, Outer = ()> {
    PsoRule::new(CFG, StdRng::seed_from_u64(seed))
}

/// An RNG whose every uniform draw in [0, 1) is exactly 0.5, however it's drawn.
struct Half;

impl RngCore for Half {
    fn next_u32(&mut self) -> u32 {
        1 << 31
    }
    fn next_u64(&mut self) -> u64 {
        1 << 63
    }
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        dst.fill(0);
    }
}

#[test]
fn one_update_follows_the_formula() {
    let x = V([1.0, -2.0, 0.5, 3.0, -1.5, 0.0]);
    let v = V([0.4, 0.1, -0.3, 0.0, 2.0, -1.0]);
    let pbest = V([2.0, -2.0, 1.5, 0.0, -1.0, 4.0]);
    let gbest = V([-1.0, 0.5, 0.5, 3.5, 2.0, -3.0]);

    let mut p = Particle::new(x, v);
    p.best_pos = pbest;
    let mut mover = PsoRule::new(CFG, Half);
    fn run(m: &mut impl Rule<Pos = V, Shared = BestSoFar<V>, Outer = ()>, g: V, p: &mut Particle<V>) {
        m.apply(&BestSoFar { best_pos: g, best_score: 0.0 }, p.view());
    }
    run(&mut mover, gbest, &mut p);

    let want_v: [f64; 6] = std::array::from_fn(|i| {
        0.5 * v.0[i] + 1.5 * 0.5 * (pbest.0[i] - x.0[i]) + 2.0 * 0.5 * (gbest.0[i] - x.0[i])
    });
    let want_x: [f64; 6] = std::array::from_fn(|i| x.0[i] + want_v[i]);
    assert!(max_abs_diff(p.vel.0, want_v) < 1e-12, "velocity {:?}, want {want_v:?}", p.vel);
    assert!(max_abs_diff(p.pos.0, want_x) < 1e-12, "position {:?}, want {want_x:?}", p.pos);
    assert_eq!(p.best_pos, pbest, "a mover doesn't change the personal best");
}

#[test]
fn at_both_bests_only_inertia_remains() {
    let x = V([1.0, -2.0, 0.5, 3.0, -1.5, 0.0]);
    let v = V([0.4, 0.1, -0.3, 0.0, 2.0, -1.0]);
    let mut p = Particle::new(x, v);
    let common = BestSoFar { best_pos: x, best_score: 0.0 };

    pso(1).apply(&common, p.view());

    assert_eq!(p.vel, v * 0.5);
    assert_eq!(p.pos, x + v * 0.5);
}

#[test]
fn random_factors_are_drawn_per_coordinate() {
    for seed in 0..10 {
        let mut p = Particle::new(V([0.0; 6]), V([0.0; 6]));
        p.best_pos = V([1.0; 6]);
        let common = BestSoFar { best_pos: V([1.0; 6]), best_score: 0.0 };

        pso(seed).apply(&common, p.view());

        // v_i = 1.5 * r1_i + 2.0 * r2_i, with r in [0, 1).
        for v in p.vel.0 {
            assert!((0.0..3.5).contains(&v), "seed {seed}: {:?}", p.vel);
        }
        assert_eq!(p.pos, p.vel);
        let first = p.vel.0[0];
        assert!(p.vel.0.iter().any(|&v| v != first), "seed {seed}: every coordinate got the same factors: {:?}", p.vel);
    }
}
