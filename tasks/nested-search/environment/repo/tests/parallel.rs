//! Results don't depend on the number of threads.

mod common;

use common::two_part::*;
use common::*;
use flockwise::routes::model::{Scenario, Wind, Zone};
use flockwise::routes::{Budget, plan};
use flockwise::{Confined, ForField, InnerSearch, Particle, PsoRule, Run, Swarm1, Swarm2};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rayon::ThreadPoolBuilder;

fn in_pool<T: Send>(threads: usize, f: impl FnOnce() -> T + Send) -> T {
    ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(f)
}

/// Runs the two-level composition from `nested.rs` and returns every
/// particle's final state and every best so far.
fn nested_run(seed: u64) -> (Vec<Particle<W>>, Vec<flockwise::BestSoFar<W>>) {
    let problem = Problem::standard();
    let inner = Swarm1::new(InnerFit::new(problem), Confined::new(PsoRule::new(CONFIG), TBounds::new(problem)));
    let mut outer = Swarm2::new(
        OuterFit(problem),
        Confined::new(PsoRule::new(CONFIG), XBounds(problem)).for_field::<X>(),
        InnerSearch::new(10, inner, TInit { problem, count: 6 }),
    );
    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&problem, 16, &mut rng);
    let bests = Run::new(8, &mut outer, &mut group, seed).collect();
    (group, bests)
}

#[test]
fn nested_search_gives_the_same_results_with_any_thread_count() {
    let seed = 17 + seed_base();
    let one = in_pool(1, || nested_run(seed));
    for threads in [2, 8] {
        let many = in_pool(threads, || nested_run(seed));
        assert!(one == many, "{threads} threads gave different results than 1");
    }
}

#[test]
fn plan_gives_the_same_route_with_any_thread_count() {
    let scenario = Scenario {
        start: [0.0, 0.0],
        goal: [16.0, 12.0],
        area: [[-4.0, -4.0], [20.0, 16.0]],
        zones: [
            Zone { center: [8.0, 6.0], radius: 2.5 },
            Zone { center: [12.0, 3.0], radius: 1.5 },
            Zone { center: [4.0, 9.0], radius: 2.0 },
        ],
        wind: Wind { base: [-1.5, 0.8], period: 25.0, ripple: 0.6 },
        max_airspeed: 3.5,
        max_leg_time: 15.0,
        energy_weight: 2.0,
    };
    let budget = Budget { particles: 12, iterations: 10, inner_particles: 6, inner_iterations: 8 };
    let one = in_pool(1, || plan(&scenario, &budget, 3));
    let four = in_pool(4, || plan(&scenario, &budget, 3));
    assert_eq!(one, four);
}
