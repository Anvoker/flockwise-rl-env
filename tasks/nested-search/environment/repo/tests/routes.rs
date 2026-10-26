//! The drone route planner, `flockwise::routes::plan`, on one scenario.

mod common;

use common::*;
use flockwise::routes::model::{Scenario, Wind, Zone};
use flockwise::routes::{Budget, plan};

const BUDGET: Budget = Budget {
    particles: 20,
    iterations: 60,
    inner_particles: 10,
    inner_iterations: 25,
};

fn scenario() -> Scenario {
    Scenario {
        start: [0.0, 0.0],
        goal: [16.0, 12.0],
        area: [[-4.0, -4.0], [20.0, 16.0]],
        zones: [
            Zone { center: [8.0, 6.0], radius: 2.5 },
            Zone { center: [12.0, 3.0], radius: 1.5 },
            Zone { center: [4.0, 9.0], radius: 2.0 },
        ],
        wind: Wind { base: [-1.5, 0.8], period: 25.0, ripple: 0.6 },
        max_airspeed: 3.5,
        max_leg_time: 15.0,
        energy_weight: 2.0,
    }
}

/// The best of three plans must score at least this. Flying the same
/// waypoints as fast as possible scores below -180, and the straight route
/// is infeasible.
const THRESHOLD: f64 = -80.0;

#[test]
fn plans_are_feasible_and_good() {
    let s = scenario();
    let base = 1000 * seed_base();
    let mut best = f64::NEG_INFINITY;

    for seed in base..base + 3 {
        let route = plan(&s, &BUDGET, seed);
        if let Err(why) = s.check(&route, 1e-3) {
            panic!("seed {seed}: infeasible route: {why}\n{route:?}");
        }
        best = best.max(s.score(&route));
    }

    assert!(best >= THRESHOLD, "best score {best}, need at least {THRESHOLD}");
}

#[test]
fn the_same_seed_gives_the_same_route() {
    let s = scenario();
    assert_eq!(plan(&s, &BUDGET, 7), plan(&s, &BUDGET, 7));
}
