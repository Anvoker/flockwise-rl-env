//! V7: every search starts fresh. A result found for one outer particle, or in
//! an earlier run, must never leak into another search.

mod common;

use common::two_part::*;
use common::*;
use nestswarm::{
    BoundedMover, Contextful, FitCalc, GBest, GBestSearcherOne, GBestSinglePMover, Mover,
    NestedMover, Particle, SearcherIter,
};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::sync::{Arc, Mutex};

fn nested(problem: Problem, seed: u64) -> impl Mover<TUnit = W, TCommon = GBest<W>, TContext = ()> {
    let inner = GBestSearcherOne::new(
        InnerFit::new(problem),
        BoundedMover::new(
            GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed)),
            TBounds::new(problem),
        ),
    );
    NestedMover::new(100, inner, TInit { problem, count: 40 }, StdRng::seed_from_u64(seed + 1))
}

#[test]
fn inner_search_for_b_does_not_reuse_the_result_for_a() {
    // A sits at the optimum, so its inner search reaches fitness near 0.
    // B is far away: everything reachable for B scores below -40. A stale or
    // shared global best would hand B the time vector found for A.
    let problem = Problem::standard();
    let (x_star, _) = problem.optimum();
    let x_b: [f64; K] = add(x_star, 4.0).map(|v| v.min(problem.x_hi[0]));

    for seed in seed_base()..seed_base() + 3 {
        let mut mover = nested(problem, seed);
        let mut a = Particle::new(W { x: X(x_star), t: T(x_star) }, W::default());
        let mut b = Particle::new(W { x: X(x_b), t: T(x_b) }, W::default());
        let common = GBest::new();

        mover.update(&common, a.as_mut());
        mover.update(&common, b.as_mut());

        let want_a: [f64; K] = std::array::from_fn(|i| x_star[i] + problem.c[i].clamp(-problem.w, problem.w));
        let want_b: [f64; K] = std::array::from_fn(|i| x_b[i] + problem.c[i].clamp(-problem.w, problem.w));
        assert!(max_abs_diff(a.pos.t.0, want_a) < 0.05, "seed {seed}: A got {:?}", a.pos.t);
        assert!(max_abs_diff(b.pos.t.0, want_b) < 0.05, "seed {seed}: B got {:?}, want {want_b:?}", b.pos.t);
        assert_eq!(b.pos.x, X(x_b), "the nested mover only writes its own part");
    }
}

/// A sphere whose center can be changed between runs.
#[derive(Clone, Debug)]
struct MovingSphere(Arc<Mutex<[f64; K]>>);

impl Contextful for MovingSphere {
    type TContext = ();
    fn set_context(&mut self, _: ()) {}
}

impl FitCalc for MovingSphere {
    type T = X;
    fn calculate_fit(&self, x: X) -> f64 {
        -sq_dist(x.0, *self.0.lock().unwrap())
    }
}

#[test]
fn repeated_runs_of_one_searcher_start_fresh() {
    let center = Arc::new(Mutex::new([1.0, 1.0, 1.0]));
    let mut searcher = GBestSearcherOne::new(
        MovingSphere(center.clone()),
        GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(1)),
    );

    let mut rng = StdRng::seed_from_u64(2);
    let group = |lo: f64, hi: f64, rng: &mut StdRng| -> Vec<Particle<X>> {
        (0..15)
            .map(|_| Particle::new(X(uniform(rng, [lo; K], [hi; K])), X::default()))
            .collect()
    };

    let mut g1 = group(0.0, 2.0, &mut rng);
    let first = SearcherIter::new(40, &mut searcher, &mut g1).last().unwrap();
    assert!(max_abs_diff(first.best_pos.0, [1.0; K]) < 1e-2);

    // Move the target far away. The old best now scores about -300, but its
    // stored fitness from the first run is near 0.
    *center.lock().unwrap() = [-9.0, -9.0, -9.0];
    let mut g2 = group(-10.0, -8.0, &mut rng);
    let second = SearcherIter::new(40, &mut searcher, &mut g2).last().unwrap();
    assert!(
        max_abs_diff(second.best_pos.0, [-9.0; K]) < 1e-2,
        "second run returned {:?}",
        second.best_pos
    );
}

#[test]
fn global_best_after_one_iteration_is_the_best_initial_position() {
    let problem = Problem::standard();
    let mut searcher = GBestSearcherOne::new(
        MovingSphere(Arc::new(Mutex::new(problem.a))),
        GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(3)),
    );
    let mut rng = StdRng::seed_from_u64(4);
    let mut group: Vec<Particle<X>> = (0..10)
        .map(|_| Particle::new(X(uniform(&mut rng, [-6.0; K], [6.0; K])), X::default()))
        .collect();
    let initial: Vec<X> = group.iter().map(|p| p.pos).collect();

    let best = SearcherIter::new(1, &mut searcher, &mut group).next().unwrap();
    let want = initial
        .iter()
        .copied()
        .max_by(|p, q| (-sq_dist(p.0, problem.a)).total_cmp(&-sq_dist(q.0, problem.a)))
        .unwrap();
    assert_eq!(best.best_pos, want);
}
