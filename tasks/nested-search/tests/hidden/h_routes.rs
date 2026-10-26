//! The route planner on other scenarios: one where getting around the zones
//! is the hard part, and one with different wind and weights where timing
//! quality is checked closely.

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

fn zones(z: [([f64; 2], f64); 3]) -> [Zone; 3] {
    z.map(|(center, radius)| Zone { center, radius })
}

/// A zone sits on the straight line. Thresholds are set from 300 runs of the
/// reference (worst best-of-3: -25.4).
fn around_the_zones() -> Scenario {
    Scenario {
        start: [0.0, 0.0],
        goal: [20.0, 0.0],
        area: [[-5.0, -12.0], [25.0, 12.0]],
        zones: zones([([10.0, 0.5], 3.0), ([4.0, -3.5], 2.0), ([15.5, 4.0], 2.0)]),
        wind: Wind { base: [1.2, 0.4], period: 40.0, ripple: 0.4 },
        max_airspeed: 3.0,
        max_leg_time: 20.0,
        energy_weight: 0.5,
    }
}

/// Light weight on energy, strong slow wind. Reference worst best-of-3: -9.85;
/// the same waypoints flown as fast as possible: -11.96 at best.
fn timing_matters() -> Scenario {
    Scenario {
        start: [0.0, 5.0],
        goal: [24.0, -5.0],
        area: [[-3.0, -12.0], [27.0, 12.0]],
        zones: zones([([6.0, 3.0], 2.0), ([12.0, 0.0], 2.5), ([18.0, -3.0], 2.0)]),
        wind: Wind { base: [0.5, -2.0], period: 60.0, ripple: 0.2 },
        max_airspeed: 4.0,
        max_leg_time: 12.0,
        energy_weight: 0.1,
    }
}

fn best_of_three(s: &Scenario) -> f64 {
    let base = 1000 * seed_base();
    let mut best = f64::NEG_INFINITY;
    for seed in base..base + 3 {
        let route = plan(s, &BUDGET, seed);
        if let Err(why) = s.check(&route, 1e-6) {
            panic!("seed {seed}: infeasible route: {why}\n{route:?}");
        }
        best = best.max(s.score(&route));
    }
    best
}

#[test]
fn gets_around_the_zones() {
    let best = best_of_three(&around_the_zones());
    assert!(best >= -35.0, "best score {best}");
}

#[test]
fn optimizes_timing() {
    let best = best_of_three(&timing_matters());
    assert!(best >= -10.5, "best score {best}");
}

#[test]
fn different_seeds_explore_differently() {
    let s = timing_matters();
    assert_ne!(plan(&s, &BUDGET, 1), plan(&s, &BUDGET, 2));
}
