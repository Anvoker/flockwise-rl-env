use super::model::{
    AirspaceRegion, LegTimeRegion, Route, RouteObjective, Scenario, TimingObjective, TimingSeeder,
    Waypoints,
};
use crate::rules::{Confined, ForField, InnerSearch};
use crate::search::Run;
use crate::swarm::{PsoParams, PsoRule, Swarm1, Swarm2};
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

    let timings = Swarm1::new(
        TimingObjective::new(s),
        Confined::new(PsoRule::new(PARAMS), LegTimeRegion::new(s)),
    );

    let mut search = Swarm2::new(
        RouteObjective::new(s),
        Confined::new(PsoRule::new(PARAMS), AirspaceRegion::new(s)).for_field::<Waypoints>(),
        InnerSearch::new(
            budget.inner_iterations,
            timings,
            TimingSeeder::new(s, budget.inner_particles),
        ),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut routes = s.initial_routes(budget.particles, &mut rng);
    Run::new(budget.iterations, &mut search, &mut routes, seed)
        .last()
        .map(|best| best.best_pos)
        .unwrap_or_else(|| s.baseline())
}
