//! V5 (C7): components written by the user (a mover, a boundary, a fitness)
//! compose with the library's own components at every position, including
//! inside part adapters and nested searches.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use hidden_common::*;
use nestswarm::{
    BoundedMover, GBestSearcherOne, GBestSearcherTwo, NestedMover, OnPart, SearcherIter,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn user_mover_inside_bounded_part_and_nested_search() {
    let problem = Problem::standard();
    let (x_star, t_star) = problem.optimum();

    for seed in seed_base()..seed_base() + 3 {
        let inner = GBestSearcherOne::new(
            InnerFit::new(problem),
            BoundedMover::new(
                ConstrictedPso::with_rng(StdRng::seed_from_u64(seed * 10)),
                TBounds::new(problem),
            ),
        );

        let mut outer = GBestSearcherTwo::new(
            OuterFit(problem),
            BoundedMover::new(ConstrictedPso::with_rng(StdRng::seed_from_u64(seed * 10 + 1)), XBounds(problem))
                .on_part::<X>(),
            NestedMover::new(30, inner, TInit { problem, count: 10 }, StdRng::seed_from_u64(seed * 10 + 2)),
        );

        let mut rng = StdRng::seed_from_u64(seed * 10 + 3);
        let mut group = initial_group(&problem, 20, &mut rng);
        let best = SearcherIter::new(60, &mut outer, &mut group).last().unwrap().best_pos;
        let dx = max_abs_diff(best.x.0, x_star);
        let dt = max_abs_diff(best.t.0, t_star);
        assert!(dx < 0.05 && dt < 0.05, "seed {seed}: {best:?}, dx {dx}, dt {dt}");
    }
}

#[test]
fn user_mover_on_a_part_without_a_boundary() {
    // Part adapter directly around a user mover, and a library-free inner level.
    let problem = Problem::standard();
    let log = Log::<W>::new();

    let inner = GBestSearcherOne::new(
        RecFit::new(InnerFit::new(problem), log.clone()),
        RecMover::new(ConstrictedPso::with_rng(StdRng::seed_from_u64(1)), log.clone()),
    );

    let mut outer = GBestSearcherTwo::new(
        OuterFit(problem),
        ConstrictedPso::with_rng(StdRng::seed_from_u64(2)).on_part::<X>(),
        NestedMover::new(4, inner, RecInit { inner: TInit { problem, count: 3 }, log: log.clone() }, StdRng::seed_from_u64(3)),
    );

    let mut rng = StdRng::seed_from_u64(4);
    let mut group = initial_group(&problem, 5, &mut rng);
    SearcherIter::new(3, &mut outer, &mut group).for_each(drop);
    check_counts(&log.events(), 3 * 4, &["fit", "move"]);
}
