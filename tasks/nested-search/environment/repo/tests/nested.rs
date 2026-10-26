//! An outer search over `x` with a nested search over `t` for every outer
//! particle. See `common/two_part.rs` for the problem.

mod common;

use common::two_part::*;
use common::*;
use flockwise::{
    Confined, OuterAware, Swarm1, Swarm2, PsoRule, InnerSearch,
    ForField, Search, Run,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// The inner search can be built on its own and handed over as a value.
fn inner_searcher(
    problem: Problem,
    seed: u64,
) -> impl Search<Pos = T> + OuterAware<Outer = W> + Clone + Send {
    Swarm1::new(
        InnerFit::new(problem),
        Confined::new(
            PsoRule::new(CONFIG, StdRng::seed_from_u64(seed)),
            TBounds::new(problem),
        ),
    )
}

fn solve(problem: Problem, seed: u64) -> W {
    let mut outer = Swarm2::new(
        OuterFit(problem),
        Confined::new(
            PsoRule::new(CONFIG, StdRng::seed_from_u64(seed + 1)),
            XBounds(problem),
        )
        .for_field::<X>(),
        InnerSearch::new(
            30,
            inner_searcher(problem, seed + 2),
            TInit { problem, count: 10 },
            StdRng::seed_from_u64(seed + 3),
        ),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&problem, 20, &mut rng);
    let best = Run::new(60, &mut outer, &mut group).last().unwrap();
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
