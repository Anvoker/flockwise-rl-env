//! A plain, single-level search.

mod common;

use common::*;
use flockwise::*;
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use std::sync::{Arc, Mutex};

vector_unit!(V, 4);

/// -|v - center|^2
fn sphere(center: [f64; 4]) -> impl Fn(V) -> f64 + Clone + Send + Sync {
    move |v: V| -sq_dist(v.0, center)
}

fn box100(v: V) -> V {
    V(clamp(v.0, [-100.0; 4], [100.0; 4]))
}

fn start(rng: &mut StdRng, count: usize) -> Vec<Particle<V>> {
    (0..count)
        .map(|_| Particle::new(V(uniform(rng, [-10.0; 4], [10.0; 4])), V(uniform(rng, [-1.0; 4], [1.0; 4]))))
        .collect()
}

#[test]
fn single_search_finds_the_center() {
    let center = [1.0, -4.0, 2.5, 7.0];

    for seed in seed_base()..seed_base() + 5 {
        // Neither component needs an outer position, so both ignore it.
        let mut searcher = Swarm1::new(
            objective(move |_| sphere(center)),
            Confined::new(PsoRule::new(CONFIG), region(|_| box100)),
        );

        let mut rng = StdRng::seed_from_u64(seed + 100);
        let mut group = start(&mut rng, 20);

        let initial: Vec<V> = group.iter().map(|p| p.pos).collect();
        let mut iter = Run::new(100, &mut searcher, &mut group, seed);
        let first = iter.next().unwrap();
        let mut bests = vec![first];
        bests.extend(iter);
        assert_eq!(bests.len(), 100, "one best so far per iteration");

        // The best so far never gets worse.
        for pair in bests.windows(2) {
            assert!(pair[1].best_score >= pair[0].best_score, "seed {seed}: the best so far got worse");
        }

        // After the first iteration, the best so far is the best starting position.
        let score = sphere(center);
        let best_start = initial
            .iter()
            .copied()
            .max_by(|a, b| score(*a).total_cmp(&score(*b)))
            .unwrap();
        assert_eq!(first.best_pos, best_start, "seed {seed}");
        assert_eq!(first.best_score, score(best_start), "seed {seed}");
        let best = *bests.last().unwrap();
        let d = max_abs_diff(best.best_pos.0, center);
        assert!(d < 1e-3, "seed {seed}: {best:?}");
    }
}

#[test]
fn personal_bests_after_one_iteration_are_the_starting_positions() {
    let center = [1.0, -4.0, 2.5, 7.0];
    let mut searcher = Swarm1::new(
        objective(move |_| sphere(center)),
        Confined::new(PsoRule::new(CONFIG), region(|_| box100)),
    );
    let mut rng = StdRng::seed_from_u64(6);
    let mut group = start(&mut rng, 8);
    let initial: Vec<V> = group.iter().map(|p| p.pos).collect();

    Run::new(1, &mut searcher, &mut group, 0).for_each(drop);

    let score = sphere(center);
    for (p, start) in group.iter().zip(&initial) {
        assert_eq!(p.best_pos, *start);
        assert_eq!(p.best_score, score(*start));
        assert_ne!(p.pos, *start, "every particle moved");
    }
}

/// A rule that moves nothing and logs particles it picks at random, about
/// one in `every`.
fn sampling_logger(every: u32, log: Arc<Mutex<Vec<V>>>) -> impl Fn(&BestSoFar<V>, &mut dyn RngCore, ParticleView<'_, V>) + Clone + Send + Sync {
    move |_: &BestSoFar<V>, rng: &mut dyn RngCore, p: ParticleView<'_, V>| {
        if rng.random_ratio(1, every) {
            log.lock().unwrap().push(*p.pos);
        }
    }
}

/// Swapping a rule that draws no random numbers for one that does changes
/// nothing else in the run.
#[test]
fn logging_at_random_does_not_change_the_run() {
    let center = [1.0, -4.0, 2.5, 7.0];
    let run = |logger: Option<u32>| {
        let log = Arc::new(Mutex::new(Vec::new()));
        let observer = {
            let log = log.clone();
            rule(move |_| {
                let log = log.clone();
                move |best: &BestSoFar<V>, rng: &mut dyn RngCore, p: ParticleView<'_, V>| {
                    if let Some(every) = logger {
                        sampling_logger(every, log.clone())(best, rng, p);
                    }
                }
            })
        };
        let mut searcher = Swarm2::new(
            objective(move |_| sphere(center)),
            observer,
            Confined::new(PsoRule::new(CONFIG), region(|_| box100)),
        );
        let mut rng = StdRng::seed_from_u64(9);
        let mut group = start(&mut rng, 12);
        let bests: Vec<BestSoFar<V>> = Run::new(20, &mut searcher, &mut group, 9).collect();
        let logged = log.lock().unwrap().len();
        (bests, group, logged)
    };

    let (quiet_bests, quiet_group, _) = run(None);
    for every in [2, 5] {
        let (bests, group, logged) = run(Some(every));
        assert!(logged > 0, "the logger never logged");
        assert!(bests == quiet_bests && group == quiet_group, "logging one particle in {every} changed the run");
    }
}
