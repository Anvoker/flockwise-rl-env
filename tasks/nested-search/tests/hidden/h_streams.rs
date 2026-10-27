//! Each component's random numbers are its own: changing how much randomness
//! one component draws never changes what any other component receives.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use flockwise::*;
use hidden_common::Same;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

/// A rule that draws `k` random numbers and moves nothing.
fn drawer<P>(k: usize) -> impl Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Clone + Send + Sync {
    move |_: &BestSoFar<P>, rng: &mut dyn RngCore, _: ParticleView<'_, P>| {
        for _ in 0..k {
            rng.next_u64();
        }
    }
}

type Outcome = (Vec<BestSoFar<W>>, Vec<Particle<W>>);

fn run_outer<S: Search<Pos = W, Outer = ()>>(mut search: S, seed: u64) -> Outcome {
    let problem = Problem::standard();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&problem, 8, &mut rng);
    let bests = Run::new(4, &mut search, &mut group, seed).collect();
    (bests, group)
}

fn assert_same_for_every_k(name: &str, run: impl Fn(usize) -> Outcome) {
    let base = run(0);
    for k in [1, 3, 8] {
        assert!(run(k).same(&base), "{name}: drawing {k} more random numbers changed the run");
    }
}

#[test]
fn a_drawing_rule_before_or_after_pso_changes_nothing() {
    let problem = Problem::standard();
    let seed = 3 + seed_base();
    assert_same_for_every_k("drawer first", |k| {
        run_outer(
            Swarm2::new(
                objective(move |_| outer_fit(problem)),
                rule(move |_| drawer::<W>(k)),
                PsoRule::new(CONFIG),
            ),
            seed,
        )
    });
    assert_same_for_every_k("drawer second", |k| {
        run_outer(
            Swarm2::new(
                objective(move |_| outer_fit(problem)),
                PsoRule::new(CONFIG),
                rule(move |_| drawer::<W>(k)),
            ),
            seed,
        )
    });
}

#[test]
fn a_drawing_rule_next_to_an_inner_search_changes_nothing() {
    let problem = Problem::standard();
    let seed = 5 + seed_base();
    let inner = move || {
        Swarm1::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
        )
    };
    assert_same_for_every_k("next to the inner search", |k| {
        run_outer(
            Swarm2::new(
                objective(move |_| outer_fit(problem)),
                rule(move |_| drawer::<X>(k)).for_field::<X>(),
                InnerSearch::new(5, inner(), seeder(t_init(problem, 4))),
            ),
            seed,
        )
    });
}

#[test]
fn a_drawing_rule_inside_the_inner_search_changes_nothing_else() {
    // The drawer sits in the inner search, second, after the inner PSO.
    let problem = Problem::standard();
    let seed = 7 + seed_base();
    assert_same_for_every_k("inside the inner search", |k| {
        let inner = Swarm2::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
            rule(move |_| drawer::<T>(k)),
        );
        run_outer(
            Swarm2::new(
                objective(move |_| outer_fit(problem)),
                Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
                InnerSearch::new(5, inner, seeder(t_init(problem, 4))),
            ),
            seed,
        )
    });
}

#[test]
fn a_seeder_that_draws_more_changes_nothing_else() {
    // The seeder makes the same particles, then draws `k` more numbers.
    let problem = Problem::standard();
    let seed = 11 + seed_base();
    assert_same_for_every_k("seeder", |k| {
        let make = t_init(problem, 4);
        let inner = Swarm1::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
        );
        run_outer(
            Swarm2::new(
                objective(move |_| outer_fit(problem)),
                Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
                InnerSearch::new(
                    5,
                    inner,
                    seeder(move |w: W, rng: &mut dyn RngCore| {
                        let particles = make(w, rng);
                        for _ in 0..k {
                            rng.next_u64();
                        }
                        particles
                    }),
                ),
            ),
            seed,
        )
    });
}
