//! Inside a nested composition, every component of an inner search is
//! prepared once for that search's outer particle, before it is used, and is
//! called the stated number of times.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use flockwise::*;
use hidden_common::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

const INNER_PARTICLES: usize = 4;
const INNER_ITERATIONS: usize = 5;
const OUTER_PARTICLES: usize = 6;
const OUTER_ITERATIONS: usize = 3;

#[test]
fn every_inner_component_is_prepared_for_its_outer_particle() {
    let problem = Problem::standard();
    let log = Log::<W>::new();

    let (l1, l2, l3) = (log.clone(), log.clone(), log.clone());
    let inner = Swarm1::new(
        objective(move |w: W| rec_fit(l1.clone(), w, inner_fit(problem, w))),
        Confined::new(
            rule(move |w: W| rec_move(l2.clone(), w, constricted_pso::<T>())),
            region(move |w: W| rec_bound(l3.clone(), w, t_bounds(problem, w))),
        ),
    );

    let outer_log = Log::<()>::new();
    let l4 = outer_log.clone();
    let mut outer = Swarm2::new(
        objective(move |o: ()| rec_fit(l4.clone(), o, outer_fit(problem))),
        Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
        InnerSearch::new(INNER_ITERATIONS, inner, seeder(rec_init(log.clone(), t_init(problem, INNER_PARTICLES)))),
    );

    let mut rng = StdRng::seed_from_u64(4);
    let mut group = initial_group(&problem, OUTER_PARTICLES, &mut rng);
    let n = Run::new(OUTER_ITERATIONS, &mut outer, &mut group, 0).count();
    assert_eq!(n, OUTER_ITERATIONS, "a search yields once per iteration");

    let events = log.events();
    let inits = events.iter().filter(|e| matches!(e, Event::Init(_))).count();
    assert_eq!(inits, OUTER_PARTICLES * OUTER_ITERATIONS, "one inner search per outer particle per iteration");
    check_counts(&events, INNER_PARTICLES * INNER_ITERATIONS, &["fit", "bound", "move"]);
    check_prepared_first(&events);

    // The top-level search is one search: prepared once, scored m * n times.
    let outer_events = outer_log.events();
    assert_eq!(outer_events.iter().filter(|e| matches!(e, Event::Prep(..))).count(), 1, "top-level preparations");
    assert_eq!(
        outer_events.iter().filter(|e| matches!(e, Event::Fit(_))).count(),
        OUTER_PARTICLES * OUTER_ITERATIONS,
        "outer scores"
    );
    assert!(matches!(outer_events[0], Event::Prep(..)), "the top-level objective was used before it was prepared");
}

#[test]
fn components_wrapped_at_depth_are_prepared_too() {
    // A rule and a region inside a confined rule, inside another one.
    let problem = Problem::standard();
    let log = Log::<W>::new();
    let outer_region_log = Log::<W>::new();

    let (l1, l2, l3) = (log.clone(), log.clone(), outer_region_log.clone());
    let inner = Swarm1::new(
        objective(move |w| inner_fit(problem, w)),
        Confined::new(
            Confined::new(
                rule(move |w: W| rec_move(l1.clone(), w, constricted_pso::<T>())),
                region(move |w: W| rec_bound(l2.clone(), w, t_bounds(problem, w))),
            ),
            region(move |w: W| rec_bound(l3.clone(), w, t_bounds(problem, w))),
        ),
    );

    let init_log = Log::<W>::new();
    let mut outer = Swarm2::new(
        objective(move |_| outer_fit(problem)),
        Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
        InnerSearch::new(INNER_ITERATIONS, inner, seeder(rec_init(init_log.clone(), t_init(problem, INNER_PARTICLES)))),
    );

    let mut rng = StdRng::seed_from_u64(8);
    let mut group = initial_group(&problem, OUTER_PARTICLES, &mut rng);
    Run::new(2, &mut outer, &mut group, 0).for_each(drop);

    let per_search = INNER_PARTICLES * INNER_ITERATIONS;
    let inits = init_log.events();
    for (extra, kinds) in [(log.events(), &["bound", "move"][..]), (outer_region_log.events(), &["bound"][..])] {
        let mut events = inits.clone();
        events.extend(extra);
        check_counts(&events, per_search, kinds);
        check_prepared_first(&events);
    }
}
