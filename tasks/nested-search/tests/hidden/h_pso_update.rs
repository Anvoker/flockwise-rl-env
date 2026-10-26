//! One PSO update, checked against the formula through a one-iteration run:
//! v <- w v + c1 r1 (pbest - x) + c2 r2 (gbest - x), x <- x + v, with r1 and
//! r2 drawn per coordinate from [0, 1).

mod common;

use common::*;
use flockwise::*;

vector_unit!(V, 6);

const CFG: PsoParams = PsoParams {
    inertia: 0.5,
    cognitive: 1.5,
    social: 2.0,
};

/// Runs one iteration of a PSO swarm that maximizes `-|v - target|^2`.
fn one_step(target: [f64; 6], group: &mut [Particle<V>], seed: u64) {
    let mut search = Swarm1::new(objective(move |_| move |v: V| -sq_dist(v.0, target)), PsoRule::new(CFG));
    Run::new(1, &mut search, group, seed).for_each(drop);
}

#[test]
fn at_both_bests_only_inertia_remains() {
    let x = V([1.0, -2.0, 0.5, 3.0, -1.5, 0.0]);
    let v = V([0.4, 0.1, -0.3, 0.0, 2.0, -1.0]);
    // A single particle is its own best and the swarm's.
    let mut group = vec![Particle::new(x, v)];
    one_step([0.0; 6], &mut group, 1);

    assert_eq!(group[0].vel, v * 0.5);
    assert_eq!(group[0].pos, x + v * 0.5);
}

/// The pull towards the personal best alone: the particle is the swarm's best
/// but remembers a better personal best from before.
#[test]
fn the_cognitive_pull_follows_the_formula() {
    let mut largest = 0.0f64;
    for seed in 0..20 {
        let x = V([0.0; 6]);
        let v = V([0.2, -0.2, 0.4, 0.0, -0.4, 0.1]);
        let mut me = Particle::new(x, v);
        me.best_pos = V([1.0; 6]);
        me.best_score = f64::INFINITY;
        let other = Particle::new(V([5.0; 6]), V::default());
        let mut group = vec![me, other];
        one_step([0.0; 6], &mut group, seed);

        let p = group[0];
        assert_eq!(p.best_pos, V([1.0; 6]), "a better personal best is kept");
        assert_eq!(p.pos, x + p.vel);
        for i in 0..6 {
            let pull = p.vel.0[i] - 0.5 * v.0[i];
            assert!((0.0..1.5).contains(&pull), "seed {seed}: coordinate {i} pulled by {pull}, outside [0, 1.5)");
            largest = largest.max(pull);
        }
        let first = p.vel.0[0] - 0.5 * v.0[0];
        assert!((1..6).any(|i| p.vel.0[i] - 0.5 * v.0[i] != first), "seed {seed}: the same factor in every coordinate");
    }
    assert!(largest > 1.2, "the cognitive pull never came near its weight: at most {largest}");
}

/// The pull towards the swarm's best alone: the particle is at its own best,
/// and another particle is the swarm's best.
#[test]
fn the_social_pull_follows_the_formula() {
    let mut largest = 0.0f64;
    for seed in 0..20 {
        let x = V([0.0; 6]);
        let v = V([0.2, -0.2, 0.4, 0.0, -0.4, 0.1]);
        let leader = Particle::new(V([1.0; 6]), V::default());
        let mut group = vec![Particle::new(x, v), leader];
        one_step([1.0; 6], &mut group, seed);

        let p = group[0];
        assert_eq!(p.pos, x + p.vel);
        for i in 0..6 {
            let pull = p.vel.0[i] - 0.5 * v.0[i];
            assert!((0.0..2.0).contains(&pull), "seed {seed}: coordinate {i} pulled by {pull}, outside [0, 2)");
            largest = largest.max(pull);
        }
        let first = p.vel.0[0] - 0.5 * v.0[0];
        assert!((1..6).any(|i| p.vel.0[i] - 0.5 * v.0[i] != first), "seed {seed}: the same factor in every coordinate");
    }
    assert!(largest > 1.6, "the social pull never came near its weight: at most {largest}");
}
