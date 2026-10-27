use super::model::{Route, Scenario, Timings, Waypoints};
use crate::api::Particle;
use crate::common::{objective, region, seeder};
use crate::rules::{Confined, ForField, InnerSearch, PsoParams, PsoRule};
use crate::swarm::{Run, Swarm1, Swarm2};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

/// How much searching `plan` does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Budget {
    pub particles: usize,
    pub iterations: usize,
    pub inner_particles: usize,
    pub inner_iterations: usize,
}

/// Plans a route: an outer search over waypoints, with an inner search over
/// leg timings for every candidate route.
pub fn plan(scenario: &Scenario, budget: &Budget, seed: u64) -> Route {
    let s = *scenario;
    let params = PsoParams { inertia: 0.7, cognitive: 1.5, social: 1.5 };

    // The timings of one candidate route: limits and scores depend on its waypoints.
    let timing_search = Swarm1::new(
        objective(move |route: Route| move |timings: Timings| s.score(&Route { waypoints: route.waypoints, timings })),
        Confined::new(
            PsoRule::new(params),
            region(move |route: Route| {
                let limits = s.leg_limits(&route.waypoints);
                move |timings: Timings| limits.confine(timings)
            }),
        ),
    );
    let count = budget.inner_particles;
    let timing_seeder = seeder(move |route: Route, rng: &mut dyn RngCore| {
        (0..count)
            .map(|_| Particle::new(s.random_timings(&route.waypoints, rng), Timings::default()))
            .collect()
    });

    let mut search = Swarm2::new(
        objective(move |_: ()| move |route: Route| s.score(&route)),
        Confined::new(
            PsoRule::new(params),
            region(move |_: ()| move |waypoints: Waypoints| s.confine_waypoints(waypoints)),
        )
        .for_field::<Waypoints>(),
        InnerSearch::new(budget.inner_iterations, timing_search, timing_seeder),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut routes = s.initial_routes(budget.particles, &mut rng);
    Run::new(budget.iterations, &mut search, &mut routes, seed)
        .last()
        .map(|best| best.best_pos)
        .unwrap_or_else(|| s.baseline())
}
