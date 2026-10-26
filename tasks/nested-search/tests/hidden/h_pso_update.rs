//! One PSO update, checked against the formula through a one-iteration run:
//! v <- w v + c1 r1 (pbest - x) + c2 r2 (gbest - x), x <- x + v, with r1 and
//! r2 drawn per coordinate from [0, 1).

mod common;

use common::*;
use flockwise::*;
use rand::RngCore;

vector_unit!(V, 6);

const CFG: PsoParams = PsoParams {
    inertia: 0.5,
    cognitive: 1.5,
    social: 2.0,
};

/// Runs one iteration of a PSO swarm that maximizes `-|v - target|^2`.
fn one_step(target: [f64; 6], group: &mut Vec<Particle<V>>, seed: u64) {
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

/// Both pulls at once. A rule written by the user first moves the particle
/// one unit below its best position, which is both its personal best and the
/// swarm's, so each coordinate's pull is `cognitive * r1 + social * r2`, in
/// [0, 3.5). It sometimes comes close to that only if both weights are used.
#[test]
fn both_pulls_follow_the_formula() {
    let mut largest = 0.0f64;
    for seed in 0..20 {
        let x = V([1.0; 6]);
        let v = V([0.2, -0.2, 0.4, 0.0, -0.4, 0.1]);
        let mut group = vec![Particle::new(x, v)];
        let step_down = rule(|_| |_: &BestSoFar<V>, _: &mut dyn RngCore, p: ParticleView<'_, V>| {
            *p.pos = *p.pos - V([1.0; 6]);
        });
        let mut search = Swarm2::new(objective(|_| |p: V| -sq_dist(p.0, [1.0; 6])), step_down, PsoRule::new(CFG));
        Run::new(1, &mut search, &mut group, seed).for_each(drop);

        let p = group[0];
        assert_eq!(p.pos, V([0.0; 6]) + p.vel);
        for i in 0..6 {
            let pull = p.vel.0[i] - 0.5 * v.0[i];
            assert!((0.0..3.5).contains(&pull), "seed {seed}: coordinate {i} pulled by {pull}, outside [0, 3.5)");
            largest = largest.max(pull);
        }
        let first = p.vel.0[0] - 0.5 * v.0[0];
        assert!((1..6).any(|i| p.vel.0[i] - 0.5 * v.0[i] != first), "seed {seed}: the same factors in every coordinate");
    }
    assert!(largest > 2.7, "the pulls never came near their combined weight: at most {largest}");
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
