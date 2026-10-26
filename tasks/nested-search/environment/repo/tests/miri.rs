//! A small nested composition, sized to run under Miri:
//! `MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-ignore-leaks" cargo +nightly miri test --test miri`

mod common;

use common::two_part::*;
use common::*;
use flockwise::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn small_nested_search_runs() {
    let problem = Problem::standard();

    let build = |iterations: usize| {
        let inner = Swarm1::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
        );

        Swarm2::new(
            objective(move |_| outer_fit(problem)),
            Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
            InnerSearch::new(iterations, inner, seeder(t_init(problem, 3))),
        )
    };

    // Built in a closure and returned, then cloned: both copies run.
    let mut a = build(3);
    let mut b = a.clone();

    let mut rng = StdRng::seed_from_u64(7);
    let mut group_a = initial_group(&problem, 3, &mut rng);
    let mut group_b = group_a.clone();

    // Two worker threads, so Miri's data race detector has something to
    // check. (Under Miri the global pool would have only one.)
    let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap();
    let best_a = pool.install(|| Run::new(3, &mut a, &mut group_a, 7).last().unwrap());
    let best_b = pool.install(|| Run::new(3, &mut b, &mut group_b, 7).last().unwrap());
    assert!(best_a.best_score.is_finite());
    assert_eq!(best_a, best_b);
}
