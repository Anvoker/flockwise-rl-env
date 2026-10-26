//! V1, V8, V9: composed searchers are ordinary values. They can be built in a
//! function, stored, cloned into independent copies and sent to other threads.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use hidden_common::*;
use flockwise::{
    Confined, OuterAware, Swarm1, Swarm2, PsoRule, InnerSearch,
    ForField, Particle, Search, Run,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn build(problem: Problem, seed: u64) -> impl Search<Pos = W> + OuterAware<Outer = ()> + Clone + Send {
    let inner = Swarm1::new(
        InnerFit::new(problem),
        Confined::new(
            PsoRule::new(CONFIG, StdRng::seed_from_u64(seed)),
            TBounds::new(problem),
        ),
    );

    Swarm2::new(
        OuterFit(problem),
        Confined::new(
            PsoRule::new(CONFIG, StdRng::seed_from_u64(seed + 1)),
            XBounds(problem),
        )
        .for_field::<X>(),
        InnerSearch::new(20, inner, TInit { problem, count: 8 }, StdRng::seed_from_u64(seed + 2)),
    )
}

struct Job<S> {
    searcher: S,
    group: Vec<Particle<W>>,
}

#[test]
fn a_stored_searcher_runs_on_another_thread() {
    let problem = Problem::standard();
    let base = seed_base() * 100;
    let mut rng = StdRng::seed_from_u64(base + 9);
    let job = Job {
        searcher: build(problem, base + 1),
        group: initial_group(&problem, 16, &mut rng),
    };
    assert_send(&job.searcher);
    assert_clone(&job.searcher);

    let handle = std::thread::spawn(move || {
        let mut job = job;
        Run::new(50, &mut job.searcher, &mut job.group).last().unwrap()
    });
    let best = handle.join().unwrap();

    let (x_star, t_star) = problem.optimum();
    assert!(max_abs_diff(best.best_pos.x.0, x_star) < 0.05, "{best:?}");
    assert!(max_abs_diff(best.best_pos.t.0, t_star) < 0.05, "{best:?}");
}

#[test]
fn clones_are_independent_even_when_interleaved() {
    // Copy A starts at the optimum, copy B far from it. Run them one iteration
    // at a time, interleaved. B's global best must always be one of B's own
    // positions: anything shared with A would show A's better positions.
    let problem = Problem::standard();
    let mut a = build(problem, 5);
    let mut b = a.clone();

    let (x_star, _) = problem.optimum();
    let mut rng = StdRng::seed_from_u64(6);
    let mut group_a: Vec<Particle<W>> = (0..6)
        .map(|_| {
            let x = X(add(x_star, 0.0));
            Particle::new(W { x, t: T(x.0) }, W::default())
        })
        .collect();
    let mut group_b: Vec<Particle<W>> = (0..6)
        .map(|_| {
            let x = X(uniform(&mut rng, [-6.0; K], [-5.0; K]));
            Particle::new(W { x, t: T(x.0) }, W::default())
        })
        .collect();

    let fitness = |w: &W| problem.fitness(w.x.0, w.t.0);
    {
        let mut it_a = Run::new(4, &mut a, &mut group_a);
        let mut it_b = Run::new(4, &mut b, &mut group_b);
        let mut best_b = Vec::new();
        for _ in 0..4 {
            it_a.next().unwrap();
            best_b.push(it_b.next().unwrap());
        }
        drop(it_a);
        drop(it_b);

        let b_best_known = group_b.iter().map(|p| p.best_score).fold(f64::NEG_INFINITY, f64::max);
        let last = best_b.last().unwrap();
        assert_eq!(last.best_score, b_best_known, "B's global best must come from B's particles");
        assert!((fitness(&last.best_pos) - last.best_score).abs() < 1e-9);
    }
}

#[test]
fn a_clone_reproduces_the_original() {
    let problem = Problem::standard();
    let mut a = build(problem, 11);
    let mut b = a.clone();
    let mut rng = StdRng::seed_from_u64(12);
    let mut group_a = initial_group(&problem, 8, &mut rng);
    let mut group_b = group_a.clone();

    let best_a = Run::new(5, &mut a, &mut group_a).last().unwrap();
    let best_b = Run::new(5, &mut b, &mut group_b).last().unwrap();
    assert_eq!(best_a, best_b);
    assert_eq!(group_a, group_b);
}
