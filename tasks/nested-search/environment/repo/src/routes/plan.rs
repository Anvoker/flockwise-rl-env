use super::model::{LegLimits, Route, Scenario, Timings, Waypoints};
use crate::api::{BestSoFar, Particle};
use crate::common::{Objective, Region, Run, Seeder};
use crate::swarm::{PsoParams, PsoRule, SearchRule, Swarm1, Swarm2};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// How much searching `plan` does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Budget {
    pub particles: usize,
    pub iterations: usize,
    pub inner_particles: usize,
    pub inner_iterations: usize,
}

struct RouteObjective(Scenario);

impl Objective for RouteObjective {
    type Pos = Route;

    fn score(&self, route: Route) -> f64 {
        self.0.score(&route)
    }
}

struct Airspace(Scenario);

impl Region for Airspace {
    type Pos = Waypoints;

    fn confine(&self, waypoints: Waypoints) -> Waypoints {
        self.0.confine_waypoints(waypoints)
    }
}

/// Scores timings for one route's waypoints.
struct TimingObjective {
    scenario: Scenario,
    route: Route,
}

impl Objective for TimingObjective {
    type Pos = Timings;

    fn score(&self, timings: Timings) -> f64 {
        self.scenario.score(&Route { waypoints: self.route.waypoints, timings })
    }
}

struct LegTimes(LegLimits);

impl Region for LegTimes {
    type Pos = Timings;

    fn confine(&self, timings: Timings) -> Timings {
        self.0.confine(timings)
    }
}

struct TimingSeeder {
    scenario: Scenario,
    count: usize,
}

impl Seeder for TimingSeeder {
    type Outer = Route;
    type Inner = Timings;

    fn seed<R: Rng + ?Sized>(&self, route: Route, rng: &mut R) -> Vec<Particle<Timings>> {
        (0..self.count)
            .map(|_| Particle::new(self.scenario.random_timings(&route.waypoints, rng), Timings::default()))
            .collect()
    }
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

    let objective = RouteObjective(s);
    let airspace = Airspace(s);

    // Every candidate route gets its own search over timings.
    let timing_rule = SearchRule {
        make_search: || {
            // TODO: the route being timed isn't available here.
            let route = Route::default();
            let timing_objective = TimingObjective { scenario: s, route };
            let legs = LegTimes(s.leg_limits(&route.waypoints));
            let mut best = BestSoFar::new();
            let mut search = Swarm1::new(
                &timing_objective,
                &mut best,
                PsoRule::new(params, &legs),
            );
            let mut rng = StdRng::seed_from_u64(seed);
            let mut particles = TimingSeeder { scenario: s, count: budget.inner_particles }.seed(route, &mut rng);
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
