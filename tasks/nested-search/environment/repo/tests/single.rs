//! A plain, single-level search.

mod common;

use common::*;
use nestswarm::{
    Boundary, BoundedMover, Contextful, FitCalc, GBestSearcherOne, GBestSinglePMover, Particle,
    SearcherIter,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

vector_unit!(V, 4);

/// -|v - center|^2
#[derive(Clone, Debug)]
struct Sphere {
    center: [f64; 4],
}

impl Contextful for Sphere {
    type TContext = ();
    fn set_context(&mut self, _: ()) {}
}

impl FitCalc for Sphere {
    type T = V;
    fn calculate_fit(&self, v: V) -> f64 {
        -sq_dist(v.0, self.center)
    }
}

#[derive(Clone, Debug)]
struct Box100;

impl Contextful for Box100 {
    type TContext = ();
    fn set_context(&mut self, _: ()) {}
}

impl Boundary for Box100 {
    type T = V;
    fn handle(&self, v: V) -> V {
        V(clamp(v.0, [-100.0; 4], [100.0; 4]))
    }
}

#[test]
fn single_search_finds_the_center() {
    let center = [1.0, -4.0, 2.5, 7.0];

    for seed in seed_base()..seed_base() + 5 {
        let mut searcher = GBestSearcherOne::new(
            Sphere { center },
            BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed)), Box100),
        );

        let mut rng = StdRng::seed_from_u64(seed + 100);
        let mut group: Vec<Particle<V>> = (0..20)
            .map(|_| {
                Particle::new(
                    V(uniform(&mut rng, [-10.0; 4], [10.0; 4])),
                    V(uniform(&mut rng, [-1.0; 4], [1.0; 4])),
                )
            })
            .collect();

        let initial: Vec<V> = group.iter().map(|p| p.pos).collect();
        let mut iter = SearcherIter::new(100, &mut searcher, &mut group);
        let first = iter.next().unwrap();
        let mut bests = vec![first];
        bests.extend(iter);
        assert_eq!(bests.len(), 100, "one global best per iteration");

        // The global best never gets worse.
        for pair in bests.windows(2) {
            assert!(pair[1].best_fit >= pair[0].best_fit, "seed {seed}: the global best got worse");
        }

        // After the first iteration, the global best is the best starting position.
        let sphere = Sphere { center };
        let best_start = initial
            .iter()
            .copied()
            .max_by(|a, b| sphere.calculate_fit(*a).total_cmp(&sphere.calculate_fit(*b)))
            .unwrap();
        assert_eq!(first.best_pos, best_start, "seed {seed}");
        assert_eq!(first.best_fit, sphere.calculate_fit(best_start), "seed {seed}");
        let best = *bests.last().unwrap();
        let d = max_abs_diff(best.best_pos.0, center);
        assert!(d < 1e-3, "seed {seed}: {best:?}");
    }
}

#[test]
fn personal_bests_after_one_iteration_are_the_starting_positions() {
    let center = [1.0, -4.0, 2.5, 7.0];
    let mut searcher = GBestSearcherOne::new(
        Sphere { center },
        BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(5)), Box100),
    );
    let mut rng = StdRng::seed_from_u64(6);
    let mut group: Vec<Particle<V>> = (0..8)
        .map(|_| Particle::new(V(uniform(&mut rng, [-10.0; 4], [10.0; 4])), V(uniform(&mut rng, [-1.0; 4], [1.0; 4]))))
        .collect();
    let initial: Vec<V> = group.iter().map(|p| p.pos).collect();

    SearcherIter::new(1, &mut searcher, &mut group).for_each(drop);

    let sphere = Sphere { center };
    for (p, start) in group.iter().zip(&initial) {
        assert_eq!(p.best_pos, *start);
        assert_eq!(p.best_fit, sphere.calculate_fit(*start));
        assert_ne!(p.pos, *start, "every particle moved");
    }
}
