//! The PSO formula on a two-part unit, and through a part adapter.

mod common;

use common::two_part::*;
use nestswarm::{GBest, GBestSinglePMover, Mover, OnPart, Particle, SearchConfigGBest};
use rand::SeedableRng;
use rand::rngs::StdRng;

const CFG: SearchConfigGBest = SearchConfigGBest {
    inertia: 0.25,
    cognitive_coeff: 1.0,
    social_coeff: 1.0,
};

#[test]
fn random_factors_differ_across_parts() {
    for seed in 0..10 {
        let mut mover = GBestSinglePMover::new(CFG, StdRng::seed_from_u64(seed));
        let mut p = Particle::new(W::default(), W::default());
        let one = W { x: X([1.0; K]), t: T([1.0; K]) };
        p.best_pos = one;
        let common = GBest { best_pos: one, best_fit: 0.0 };

        fn run(m: &mut impl Mover<TUnit = W, TCommon = GBest<W>, TContext = ()>, c: &GBest<W>, p: &mut Particle<W>) {
            m.update(c, p.as_mut());
        }
        run(&mut mover, &common, &mut p);

        let all: Vec<f64> = p.vel.x.0.iter().chain(p.vel.t.0.iter()).copied().collect();
        assert!(all.iter().all(|v| (0.0..2.0).contains(v)), "seed {seed}: {:?}", p.vel);
        assert!(all.iter().any(|&v| v != all[0]), "seed {seed}: {:?}", p.vel);
    }
}

#[test]
fn a_part_mover_moves_only_its_part_towards_the_projected_best() {
    let mut mover = GBestSinglePMover::new(CFG, StdRng::seed_from_u64(3)).on_part::<X>();
    let x0 = W { x: X([2.0; K]), t: T([5.0; K]) };
    let v0 = W { x: X([0.4; K]), t: T([0.7; K]) };
    let mut p = Particle::new(x0, v0);
    let common = GBest { best_pos: x0, best_fit: 0.0 };

    fn run(m: &mut impl Mover<TUnit = W, TCommon = GBest<W>, TContext = ()>, c: &GBest<W>, p: &mut Particle<W>) {
        m.update(c, p.as_mut());
    }
    run(&mut mover, &common, &mut p);

    assert_eq!(p.vel.x, X([0.4; K]) * 0.25);
    assert_eq!(p.pos.x, X([2.0; K]) + X([0.4; K]) * 0.25);
    assert_eq!(p.vel.t, v0.t, "the other part's velocity is untouched");
    assert_eq!(p.pos.t, x0.t, "the other part's position is untouched");
}
