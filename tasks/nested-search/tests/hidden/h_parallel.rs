//! Parallelism and reproducibility: the work really runs on several threads,
//! results are identical for any thread count, and ties are broken the same
//! way every time.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use flockwise::routes::model::{Scenario, Wind, Zone};
use flockwise::routes::{Budget, plan};
use flockwise::*;
use hidden_common::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rayon::ThreadPoolBuilder;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Instant;

fn in_pool<T: Send>(threads: usize, f: impl FnOnce() -> T + Send) -> T {
    ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(f)
}

type Seen = Arc<Mutex<HashSet<ThreadId>>>;

/// Wraps a scoring function and records which threads call it.
fn seen_by<P>(seen: Seen, f: impl Fn(P) -> f64 + Clone + Send + Sync) -> impl Fn(P) -> f64 + Clone + Send + Sync {
    move |pos: P| {
        seen.lock().unwrap().insert(std::thread::current().id());
        f(pos)
    }
}

fn nested(outer_seen: &Seen, inner_seen: &Seen, inner_iterations: usize) -> impl Search<Pos = W, Outer = ()> {
    let problem = Problem::standard();
    let (outer_seen, inner_seen) = (outer_seen.clone(), inner_seen.clone());
    let inner = Swarm1::new(
        objective(move |w| seen_by(inner_seen.clone(), inner_fit(problem, w))),
        Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
    );
    Swarm2::new(
        objective(move |_| seen_by(outer_seen.clone(), outer_fit(problem))),
        Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
        InnerSearch::new(inner_iterations, inner, seeder(t_init(problem, 8))),
    )
}

#[test]
fn work_runs_on_several_threads() {
    let outer_seen = Seen::default();
    let inner_seen = Seen::default();
    in_pool(8, || {
        let mut search = nested(&outer_seen, &inner_seen, 40);
        let mut rng = StdRng::seed_from_u64(1);
        let mut group = initial_group(&Problem::standard(), 64, &mut rng);
        Run::new(3, &mut search, &mut group, 1).for_each(drop);
    });
    let outer = outer_seen.lock().unwrap().len();
    let inner = inner_seen.lock().unwrap().len();
    assert!(outer > 1, "outer positions were scored on {outer} thread(s) in an 8-thread pool");
    assert!(inner > 1, "inner searches ran on {inner} thread(s) in an 8-thread pool");
}

/// Every particle's final state and every best so far.
fn nested_run(seed: u64, particles: usize, iterations: usize, inner_iterations: usize) -> (Vec<Particle<W>>, Vec<BestSoFar<W>>) {
    let seen = Seen::default();
    let mut search = nested(&seen, &seen, inner_iterations);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&Problem::standard(), particles, &mut rng);
    let bests = Run::new(iterations, &mut search, &mut group, seed).collect();
    (group, bests)
}

#[test]
fn nested_search_gives_the_same_results_with_any_thread_count() {
    for (seed, particles, iterations, inner_iterations) in [(17, 16, 8, 10), (5, 12, 6, 6)] {
        let seed = seed + seed_base();
        let one = in_pool(1, || nested_run(seed, particles, iterations, inner_iterations));
        for threads in [2, 3, 8] {
            let many = in_pool(threads, || nested_run(seed, particles, iterations, inner_iterations));
            assert!(one == many, "seed {seed}: {threads} threads gave different results than 1");
        }
    }
}

#[test]
fn a_user_rule_gives_the_same_results_with_any_thread_count() {
    let run = |seed: u64| {
        let problem = Problem::standard();
        let inner = Swarm1::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(rule(|_| constricted_pso::<T>()), region(move |w| t_bounds(problem, w))),
        );
        let mut search = Swarm2::new(
            objective(move |_| outer_fit(problem)),
            Confined::new(rule(|_| constricted_pso::<X>()), region(move |_| x_bounds(problem))).for_field::<X>(),
            InnerSearch::new(5, inner, seeder(t_init(problem, 5))),
        );
        let mut rng = StdRng::seed_from_u64(seed);
        let mut group = initial_group(&problem, 10, &mut rng);
        Run::new(5, &mut search, &mut group, seed).for_each(drop);
        group
    };
    let seed = 9 + seed_base();
    assert!(in_pool(1, || run(seed)) == in_pool(6, || run(seed)));
}

fn scenario() -> Scenario {
    Scenario {
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
    }
}

#[test]
fn plan_gives_the_same_route_with_any_thread_count() {
    let budget = Budget { particles: 12, iterations: 10, inner_particles: 6, inner_iterations: 8 };
    let s = scenario();
    let one = in_pool(1, || plan(&s, &budget, 3));
    let four = in_pool(4, || plan(&s, &budget, 3));
    assert_eq!(one, four);
}

#[test]
fn ties_go_to_the_earliest_particle_with_any_thread_count() {
    for threads in [1, 2, 8] {
        let best = in_pool(threads, || {
            // Every position scores the same, so the earliest particle must win.
            let mut search = Swarm1::new(objective(|_| |_: X| 1.0), PsoRule::new(CONFIG));
            let mut group: Vec<Particle<X>> =
                (0..32).map(|i| Particle::new(X([i as f64; K]), X::default())).collect();
            Run::new(1, &mut search, &mut group, 0).next().unwrap()
        });
        assert_eq!(best.best_pos, X([0.0; K]), "{threads} threads");
    }
}

/// Not scored: prints the speedup of 8 threads over 1 for the verifier's log,
/// timing the route planner with a larger budget.
#[test]
#[ignore]
fn speedup_report() {
    let s = scenario();
    let budget = Budget { particles: 64, iterations: 6, inner_particles: 20, inner_iterations: 40 };
    let time = |threads: usize| {
        let start = Instant::now();
        in_pool(threads, || plan(&s, &budget, 1));
        start.elapsed().as_secs_f64()
    };
    let (one, eight) = (time(1), time(8));
    println!("SPEEDUP_8_VS_1 {:.2} ({one:.2} s with 1 thread)", one / eight);
}
