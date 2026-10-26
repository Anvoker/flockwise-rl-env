//! V6 and V12: inside a nested composition, every component of an inner search
//! sees that search's outer particle, for exactly the stated number of calls.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use hidden_common::*;
use flockwise::{
    Confined, Swarm1, Swarm2, PsoRule, InnerSearch, ForField,
    Run,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

const INNER_PARTICLES: usize = 4;
const INNER_ITERATIONS: usize = 5;
const OUTER_PARTICLES: usize = 6;
const OUTER_ITERATIONS: usize = 3;

#[test]
fn every_inner_component_sees_its_outer_particle() {
    let problem = Problem::standard();
    let log = Log::<W>::new();

    let inner = Swarm1::new(
        RecFit::new(InnerFit::new(problem), log.clone()),
        Confined::new(
            RecMover::new(PsoRule::new(CONFIG, StdRng::seed_from_u64(1)), log.clone()),
            RecBound::new(TBounds::new(problem), log.clone()),
        ),
    );

    let outer_fit_log = Log::<()>::new();
    let mut outer = Swarm2::new(
        RecFit::new(OuterFit(problem), outer_fit_log.clone()),
        Confined::new(PsoRule::new(CONFIG, StdRng::seed_from_u64(2)), XBounds(problem))
            .for_field::<X>(),
        InnerSearch::new(
            INNER_ITERATIONS,
            inner,
            RecInit {
                inner: TInit { problem, count: INNER_PARTICLES },
                log: log.clone(),
            },
            StdRng::seed_from_u64(3),
        ),
    );

    let mut rng = StdRng::seed_from_u64(4);
    let mut group = initial_group(&problem, OUTER_PARTICLES, &mut rng);
    let n = Run::new(OUTER_ITERATIONS, &mut outer, &mut group).count();
    assert_eq!(n, OUTER_ITERATIONS, "a search yields once per iteration");

    let events = log.events();
    let inits = events.iter().filter(|e| matches!(e, Event::Init(_))).count();
    assert_eq!(inits, OUTER_PARTICLES * OUTER_ITERATIONS, "one inner search per outer particle per iteration");
    check_counts(&events, INNER_PARTICLES * INNER_ITERATIONS, &["fit", "bound", "move"]);

    let outer_evals = outer_fit_log.events().len();
    assert_eq!(outer_evals, OUTER_PARTICLES * OUTER_ITERATIONS, "outer fitness evaluations");
}

#[test]
fn context_reaches_components_wrapped_at_depth() {
    // Recorders at two depths: around the bounded mover, and inside it.
    let problem = Problem::standard();
    let log = Log::<W>::new();
    let wrapper_log = Log::<W>::new();
    let init_log = Log::<W>::new();

    let inner = Swarm1::new(
        InnerFit::new(problem),
        RecMover::new(
            Confined::new(
                RecMover::new(PsoRule::new(CONFIG, StdRng::seed_from_u64(5)), log.clone()),
                RecBound::new(TBounds::new(problem), log.clone()),
            ),
            wrapper_log.clone(),
        ),
    );

    let mut outer = Swarm2::new(
        OuterFit(problem),
        Confined::new(PsoRule::new(CONFIG, StdRng::seed_from_u64(6)), XBounds(problem))
            .for_field::<X>(),
        InnerSearch::new(
            INNER_ITERATIONS,
            inner,
            RecInit { inner: TInit { problem, count: INNER_PARTICLES }, log: init_log.clone() },
            StdRng::seed_from_u64(7),
        ),
    );

    let mut rng = StdRng::seed_from_u64(8);
    let mut group = initial_group(&problem, OUTER_PARTICLES, &mut rng);
    Run::new(2, &mut outer, &mut group).for_each(drop);

    let per_search = INNER_PARTICLES * INNER_ITERATIONS;
    let inits = init_log.events();
    for extra in [log.events(), wrapper_log.events()] {
        let mut events = inits.clone();
        events.extend(extra);
        let kinds: &[&str] = if events.iter().any(|e| matches!(e, Event::Bound(_))) {
            &["bound", "move"]
        } else {
            &["move"]
        };
        check_counts(&events, per_search, kinds);
    }
}
