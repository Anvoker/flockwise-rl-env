//! An outer search over `x` with a nested search over `t` for every outer
//! particle. See `common/two_part.rs` for the problem.

mod common;

use common::two_part::*;
use common::*;
use flockwise::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The inner search can be built on its own and handed over as a value. Its
/// components borrow the problem instead of copying it.
fn inner_search(problem: &Problem) -> impl Search<Pos = T, Outer = W> + Clone + Send {
    Swarm1::new(
        objective(move |w| inner_fit(*problem, w)),
        Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(*problem, w))),
    )
}

fn solve(problem: Problem, seed: u64) -> W {
    let mut outer = Swarm2::new(
        objective(move |_| outer_fit(problem)),
        Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
        InnerSearch::new(30, inner_search(&problem), seeder(t_init(problem, 10))),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&problem, 20, &mut rng);
    let best = Run::new(60, &mut outer, &mut group, seed).last().unwrap();
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

/// A component's closure is called once for every search the component takes
/// part in: once for the top-level search, and once for each inner search.
#[test]
fn closures_are_called_once_per_search() {
    let problem = Problem::standard();
    let (outer_calls, inner_calls) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));

    let counted_outer = {
        let calls = outer_calls.clone();
        objective(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            outer_fit(problem)
        })
    };
    let counted_inner = {
        let calls = inner_calls.clone();
        objective(move |w| {
            calls.fetch_add(1, Ordering::SeqCst);
            inner_fit(problem, w)
        })
    };

    let inner = Swarm1::new(
        counted_inner,
        Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
    );
    let mut outer = Swarm2::new(
        counted_outer,
        Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
        InnerSearch::new(4, inner, seeder(t_init(problem, 3))),
    );

    let mut rng = StdRng::seed_from_u64(1);
    let mut group = initial_group(&problem, 5, &mut rng);
    Run::new(6, &mut outer, &mut group, 1).for_each(drop);

    assert_eq!(outer_calls.load(Ordering::SeqCst), 1, "one top-level search");
    assert_eq!(inner_calls.load(Ordering::SeqCst), 5 * 6, "one inner search per particle per iteration");
}
