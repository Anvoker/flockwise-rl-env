use super::model::{Route, Scenario, Timings, Waypoints};
use crate::components::{objective, region, seeder};
use crate::rules::{Confined, ForField, InnerSearch};
use crate::search::Run;
use crate::swarm::{PsoParams, PsoRule, Swarm1, Swarm2};
use crate::api::Particle;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// How much searching `plan` does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Budget {
    pub particles: usize,
    pub iterations: usize,
    pub inner_particles: usize,
    pub inner_iterations: usize,
}

const PARAMS: PsoParams = PsoParams {
    inertia: 0.7,
    cognitive: 1.5,
    social: 1.5,
};

/// Plans a route: an outer search over waypoints, with an inner search over
/// leg timings for every candidate route.
pub fn plan(scenario: &Scenario, budget: &Budget, seed: u64) -> Route {
    let s = *scenario;
    let count = budget.inner_particles;

    let timings = Swarm1::new(
        objective(move |route: Route| move |timings: Timings| s.score(&Route { waypoints: route.waypoints, timings })),
        Confined::new(
            PsoRule::new(PARAMS),
            region(move |route: Route| {
                let limits = s.leg_limits(&route.waypoints);
                move |timings: Timings| limits.confine(timings)
            }),
        ),
    );

    let mut search = Swarm2::new(
        objective(move |_| move |route: Route| s.score(&route)),
        Confined::new(
            PsoRule::new(PARAMS),
            region(move |_| move |waypoints: Waypoints| s.confine_waypoints(waypoints)),
        )
        .for_field::<Waypoints>(),
        InnerSearch::new(
            budget.inner_iterations,
            timings,
            seeder(move |route: Route, rng| {
                (0..count)
                    .map(|_| Particle::new(s.random_timings(&route.waypoints, rng), Timings::default()))
                    .collect()
            }),
        ),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut routes = s.initial_routes(budget.particles, &mut rng);
    Run::new(budget.iterations, &mut search, &mut routes, seed)
        .last()
        .map(|best| best.best_pos)
        .unwrap_or_else(|| s.baseline())
}
