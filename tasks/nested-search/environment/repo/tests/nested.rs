//! An outer search over `x` with a nested search over `t` for every outer
//! particle. See `common/two_part.rs` for the problem.

mod common;

use common::two_part::*;
use common::*;
use nestswarm::{
    BoundedMover, Contextful, GBestSearcherOne, GBestSearcherTwo, GBestSinglePMover, NestedMover,
    OnPart, Searcher, SearcherIter,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// The inner search can be built on its own and handed over as a value.
fn inner_searcher(
    problem: Problem,
    seed: u64,
) -> impl Searcher<TUnit = T> + Contextful<TContext = W> + Clone + Send {
    GBestSearcherOne::new(
        InnerFit::new(problem),
        BoundedMover::new(
            GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed)),
            TBounds::new(problem),
        ),
    )
}

fn solve(problem: Problem, seed: u64) -> W {
    let mut outer = GBestSearcherTwo::new(
        OuterFit(problem),
        BoundedMover::new(
            GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed + 1)),
            XBounds(problem),
        )
        .on_part::<X>(),
        NestedMover::new(
            30,
            inner_searcher(problem, seed + 2),
            TInit { problem, count: 10 },
            StdRng::seed_from_u64(seed + 3),
        ),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&problem, 20, &mut rng);
    let best = SearcherIter::new(60, &mut outer, &mut group).last().unwrap();
    best.best_pos
}

#[test]
fn nested_search_finds_the_joint_optimum() {
    let problem = Problem::standard();
    let (x_star, t_star) = problem.optimum();

    for seed in seed_base()..seed_base() + 5 {
        let best = solve(problem, seed * 10);
        let dx = max_abs_diff(best.x.0, x_star);
        let dt = max_abs_diff(best.t.0, t_star);
        assert!(dx < 0.05 && dt < 0.05, "seed {seed}: {best:?}, dx {dx}, dt {dt}");
    }
}
