use super::model::{Route, Scenario, Timings, Waypoints};
use crate::api::Particle;
use crate::combine::{Confined, InnerSearch};
use crate::common::{Rule, Run, Search, objective, region, seeder};
use crate::swarm::{PsoParams, PsoRule, Swarm1, Swarm2};
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

/// Searches leg timings for the waypoints of one candidate route.
fn timing_search(s: Scenario, params: PsoParams) -> impl Search<Pos = Timings, Outer = Route> + Clone + Send {
    Swarm1::new(
        objective(move |route: Route| move |t: Timings| s.score(&Route { waypoints: route.waypoints, timings: t })),
        Confined::new(
            PsoRule::new(params),
            region(move |route: Route| {
                let limits = s.leg_limits(&route.waypoints);
                move |t: Timings| limits.confine(t)
            }),
        ),
    )
}

/// Plans a route: an outer search over waypoints, with an inner search over
/// leg timings for every candidate route.
pub fn plan(scenario: &Scenario, budget: &Budget, seed: u64) -> Route {
    let s = *scenario;
    let params = PsoParams {
        inertia: 0.7,
        cognitive: 1.5,
        social: 1.5,
    };

    let timings = InnerSearch::new(
        budget.inner_iterations,
        timing_search(s, params),
        seeder(move |route: Route, rng: &mut dyn RngCore| {
            (0..budget.inner_particles)
                .map(|_| Particle::new(s.random_timings(&route.waypoints, rng), Timings::default()))
                .collect()
        }),
    );

    let mut search = Swarm2::new(
        objective(move |_| move |route: Route| s.score(&route)),
        Confined::new(
            PsoRule::new(params),
            region(move |_| move |waypoints: Waypoints| s.confine_waypoints(waypoints)),
        )
        .for_field::<Waypoints>(),
        timings,
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut routes = s.initial_routes(budget.particles, &mut rng);
    Run::new(budget.iterations, &mut search, &mut routes, seed)
        .last()
        .map(|best| best.best_pos)
        .unwrap_or_else(|| s.baseline())
}
