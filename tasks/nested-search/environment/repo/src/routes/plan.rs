use super::model::{
    AirspaceRegion, LegTimeRegion, Route, RouteObjective, Scenario, TimingObjective, TimingSeeder,
};
use crate::api::{BestSoFar, Seeder};
use crate::common::Run;
use crate::swarm::{PsoParams, PsoRule, SearchRule, Swarm1, Swarm2};
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

/// Plans a route: an outer search over waypoints, with an inner search over
/// leg timings for every candidate route.
pub fn plan(scenario: &Scenario, budget: &Budget, seed: u64) -> Route {
    let s = *scenario;
    let params = PsoParams {
        inertia: 0.7,
        cognitive: 1.5,
        social: 1.5,
    };

    let objective = RouteObjective::new(s);
    let airspace = AirspaceRegion::new(s);

    // Every candidate route gets its own search over timings.
    let timing_rule = SearchRule {
        make_search: || {
            let timing_objective = TimingObjective::new(s);
            let legs = LegTimeRegion::new(s);
            let mut best = BestSoFar::new();
            let mut search = Swarm1::new(
                &timing_objective,
                &mut best,
                PsoRule::new(params, &legs),
            );
            // TODO: the route being timed isn't available here.
            let mut rng = StdRng::seed_from_u64(seed);
            let mut particles = TimingSeeder::new(s, budget.inner_particles).seed(Route::default(), &mut rng);
            Run::new(budget.inner_iterations, &mut search, &mut particles)
        },
    };

    let mut best = BestSoFar::new();
    let mut search = Swarm2::new(
        &objective,
        &mut best,
        PsoRule::new(params, &airspace),
        timing_rule,
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut routes = s.initial_routes(budget.particles, &mut rng);
    Run::new(budget.iterations, &mut search, &mut routes)
        .last()
        .map(|best| best.best_pos)
        .unwrap_or_else(|| s.baseline())
}
