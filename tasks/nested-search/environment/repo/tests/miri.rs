//! A small nested composition, sized to run under Miri:
//! `cargo +nightly miri test --test miri`

mod common;

use common::two_part::*;
use common::*;
use nestswarm::{
    BoundedMover, GBestSearcherOne, GBestSearcherTwo, GBestSinglePMover, NestedMover, OnPart,
    SearcherIter,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn small_nested_search_runs() {
    let problem = Problem::standard();

    let build = |seed: u64| {
        let inner = GBestSearcherOne::new(
            InnerFit::new(problem),
            BoundedMover::new(
                GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed)),
                TBounds::new(problem),
            ),
        );

        GBestSearcherTwo::new(
            OuterFit(problem),
            BoundedMover::new(
                GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed + 1)),
                XBounds(problem),
            )
            .on_part::<X>(),
            NestedMover::new(3, inner, TInit { problem, count: 3 }, StdRng::seed_from_u64(seed + 2)),
        )
    };

    // Built in a closure and returned, then cloned: both copies run.
    let mut a = build(1);
    let mut b = a.clone();

    let mut rng = StdRng::seed_from_u64(7);
    let mut group_a = initial_group(&problem, 3, &mut rng);
    let mut group_b = group_a.clone();

    let best_a = SearcherIter::new(3, &mut a, &mut group_a).last().unwrap();
    let best_b = SearcherIter::new(3, &mut b, &mut group_b).last().unwrap();
    assert!(best_a.best_fit.is_finite());
    assert_eq!(best_a, best_b);
}
