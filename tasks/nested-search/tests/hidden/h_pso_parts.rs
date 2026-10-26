//! The PSO formula on a two-part position, and on one field of it.

mod common;

use common::two_part::*;
use common::*;
use flockwise::*;

const CFG: PsoParams = PsoParams {
    inertia: 0.25,
    cognitive: 1.0,
    social: 1.0,
};

fn one() -> W {
    W { x: X([1.0; K]), t: T([1.0; K]) }
}

#[test]
fn random_factors_differ_across_fields() {
    for seed in 0..10 {
        // Particle 0 starts at zero with a better personal best at `one`, and
        // particle 1 sits at `one`, the swarm's best.
        let mut me = Particle::new(W::default(), W::default());
        me.best_pos = one();
        me.best_score = f64::INFINITY;
        let mut group = vec![me, Particle::new(one(), W::default())];
        let target = one();
        let mut search = Swarm1::new(
            objective(move |_| move |w: W| -sq_dist(w.x.0, target.x.0) - sq_dist(w.t.0, target.t.0)),
            PsoRule::new(CFG),
        );
        Run::new(1, &mut search, &mut group, seed).for_each(drop);

        let p = group[0];
        let all: Vec<f64> = p.vel.x.0.iter().chain(p.vel.t.0.iter()).copied().collect();
        assert!(all.iter().all(|v| (0.0..2.0).contains(v)), "seed {seed}: {:?}", p.vel);
        assert!(all.iter().any(|&v| v != all[0]), "seed {seed}: {:?}", p.vel);
    }
}

#[test]
fn a_field_rule_moves_only_its_field() {
    let x0 = W { x: X([2.0; K]), t: T([5.0; K]) };
    let v0 = W { x: X([0.4; K]), t: T([0.7; K]) };
    // A single particle: at both bests, only inertia remains.
    let mut group = vec![Particle::new(x0, v0)];
    let mut search = Swarm1::new(objective(|_| |_: W| 0.0), PsoRule::new(CFG).for_field::<X>());
    Run::new(1, &mut search, &mut group, 3).for_each(drop);

    let p = group[0];
    assert_eq!(p.vel.x, X([0.4; K]) * 0.25);
    assert_eq!(p.pos.x, X([2.0; K]) + X([0.4; K]) * 0.25);
    assert_eq!(p.vel.t, v0.t, "the other field's velocity is untouched");
    assert_eq!(p.pos.t, x0.t, "the other field's position is untouched");
}

#[test]
fn a_field_rule_is_pulled_towards_that_field_of_the_best_so_far() {
    for seed in 0..10 {
        // Particle 1 is the swarm's best. Its `x` is ahead of particle 0's in
        // every coordinate, and its `t` is behind, so a rule shown the wrong
        // field of the best so far moves the wrong way.
        let leader = W { x: X([1.0; K]), t: T([-1.0; K]) };
        let mut group = vec![Particle::new(W::default(), W::default()), Particle::new(leader, W::default())];
        let mut search = Swarm1::new(
            objective(move |_| move |w: W| -sq_dist(w.x.0, leader.x.0) - sq_dist(w.t.0, leader.t.0)),
            PsoRule::new(CFG).for_field::<X>(),
        );
        Run::new(1, &mut search, &mut group, seed).for_each(drop);

        let p = group[0];
        assert!(p.vel.x.0.iter().all(|v| (0.0..1.0).contains(v)), "seed {seed}: {:?}", p.vel.x);
        assert!(p.vel.x.0.iter().any(|&v| v > 0.0), "seed {seed}: {:?}", p.vel.x);
        assert_eq!(p.pos.t, T::default(), "seed {seed}: the other field moved");
    }
}
