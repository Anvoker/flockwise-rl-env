//! Parallelism and reproducibility: the work really runs on several threads,
//! results are identical for any thread count, and ties are broken the same
//! way every time.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use flockwise::{
    BestSoFar, Confined, ForField, InnerSearch, Objective, OuterAware, Particle, PsoRule, Run,
    Swarm1, Swarm2,
};
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

/// Records which threads score positions.
#[derive(Clone, Debug)]
struct ThreadsSeen<F> {
    inner: F,
    seen: Arc<Mutex<HashSet<ThreadId>>>,
}

impl<F: OuterAware> OuterAware for ThreadsSeen<F> {
    type Outer = F::Outer;
    fn see_outer(&mut self, outer: Self::Outer) {
        self.inner.see_outer(outer);
    }
}

impl<F: Objective> Objective for ThreadsSeen<F> {
    type Pos = F::Pos;
    fn score(&self, pos: Self::Pos) -> f64 {
        self.seen.lock().unwrap().insert(std::thread::current().id());
        self.inner.score(pos)
    }
}

fn nested(
    outer_seen: &Arc<Mutex<HashSet<ThreadId>>>,
    inner_seen: &Arc<Mutex<HashSet<ThreadId>>>,
    inner_iterations: usize,
) -> impl flockwise::Search<Pos = W> + OuterAware<Outer = ()> {
    let problem = Problem::standard();
    let inner = Swarm1::new(
        ThreadsSeen { inner: InnerFit::new(problem), seen: inner_seen.clone() },
        Confined::new(PsoRule::new(CONFIG), TBounds::new(problem)),
    );
    Swarm2::new(
        ThreadsSeen { inner: OuterFit(problem), seen: outer_seen.clone() },
        Confined::new(PsoRule::new(CONFIG), XBounds(problem)).for_field::<X>(),
        InnerSearch::new(inner_iterations, inner, TInit { problem, count: 8 }),
    )
}

#[test]
fn work_runs_on_several_threads() {
    let outer_seen = Arc::new(Mutex::new(HashSet::new()));
    let inner_seen = Arc::new(Mutex::new(HashSet::new()));
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

/// A nested composition of another size, run with different thread counts.
#[test]
fn nested_search_gives_the_same_results_with_any_thread_count() {
    let run = |seed: u64| {
        let problem = Problem::standard();
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let mut search = nested(&seen, &seen, 6);
        let mut rng = StdRng::seed_from_u64(seed);
        let mut group = initial_group(&problem, 12, &mut rng);
        let bests: Vec<BestSoFar<W>> = Run::new(6, &mut search, &mut group, seed).collect();
        (group, bests)
    };
    let seed = 5 + seed_base();
    let one = in_pool(1, || run(seed));
    for threads in [3, 8] {
        assert!(one == in_pool(threads, || run(seed)), "{threads} threads differ from 1");
    }
}

#[test]
fn a_user_rule_gives_the_same_results_with_any_thread_count() {
    let run = |seed: u64| {
        let problem = Problem::standard();
        let inner = Swarm1::new(InnerFit::new(problem), Confined::new(ConstrictedPso::new(), TBounds::new(problem)));
        let mut search = Swarm2::new(
            OuterFit(problem),
            Confined::new(ConstrictedPso::new(), XBounds(problem)).for_field::<X>(),
            InnerSearch::new(5, inner, TInit { problem, count: 5 }),
        );
        let mut rng = StdRng::seed_from_u64(seed);
        let mut group = initial_group(&problem, 10, &mut rng);
        Run::new(5, &mut search, &mut group, seed).for_each(drop);
        group
    };
    let seed = 9 + seed_base();
    assert!(in_pool(1, || run(seed)) == in_pool(6, || run(seed)));
}

/// Every position scores the same, so the earliest particle must win.
#[derive(Clone, Debug)]
struct Flat;

impl OuterAware for Flat {
    type Outer = ();
    fn see_outer(&mut self, _: ()) {}
}

impl Objective for Flat {
    type Pos = X;
    fn score(&self, _: X) -> f64 {
        1.0
    }
}

#[test]
fn ties_go_to_the_earliest_particle_with_any_thread_count() {
    for threads in [1, 2, 8] {
        let best = in_pool(threads, || {
            let mut search = Swarm1::new(Flat, PsoRule::new(CONFIG));
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
    use flockwise::routes::model::{Scenario, Wind, Zone};
    use flockwise::routes::{Budget, plan};

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
    let budget = Budget { particles: 64, iterations: 6, inner_particles: 20, inner_iterations: 40 };
    let time = |threads: usize| {
        let start = Instant::now();
        in_pool(threads, || plan(&scenario, &budget, 1));
        start.elapsed().as_secs_f64()
    };
    let (one, eight) = (time(1), time(8));
    println!("SPEEDUP_8_VS_1 {:.2} ({one:.2} s with 1 thread)", one / eight);
}
