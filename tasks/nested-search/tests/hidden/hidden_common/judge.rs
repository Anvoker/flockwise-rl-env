//! The verifier's own judges, independent of the agent's crate.
//!
//! The agent's `lib.rs` decides which files are compiled, so the restored
//! copies of `api.rs` and `routes/model.rs` aren't guaranteed to be the code
//! that runs. The hidden tests therefore compare results field by field
//! instead of with the library's `PartialEq`, and score routes with this copy
//! of the model instead of with `Scenario::score` and `Scenario::check`. It
//! reads only the public data fields, and follows `routes/model.rs` operation
//! for operation, so it gives bit-identical results to the real model.
#![allow(dead_code)]

use flockwise::routes::model::{Route, Scenario, Zone};
use flockwise::{BestSoFar, Particle};
use std::f64::consts::TAU;

/// Equality of results, decided by the tests rather than the library.
/// Positions are the tests' own types, compared with their own `PartialEq`;
/// scores are compared bit for bit.
pub trait Same {
    fn same(&self, other: &Self) -> bool;
}

impl<P: PartialEq> Same for Particle<P> {
    fn same(&self, o: &Self) -> bool {
        self.pos == o.pos
            && self.vel == o.vel
            && self.best_pos == o.best_pos
            && self.score.to_bits() == o.score.to_bits()
            && self.best_score.to_bits() == o.best_score.to_bits()
    }
}

impl<P: PartialEq> Same for BestSoFar<P> {
    fn same(&self, o: &Self) -> bool {
        self.best_pos == o.best_pos && self.best_score.to_bits() == o.best_score.to_bits()
    }
}

impl<T: Same> Same for [T] {
    fn same(&self, o: &Self) -> bool {
        self.len() == o.len() && self.iter().zip(o).all(|(a, b)| a.same(b))
    }
}

impl<T: Same> Same for Vec<T> {
    fn same(&self, o: &Self) -> bool {
        self.as_slice().same(o.as_slice())
    }
}

impl<A: Same, B: Same> Same for (A, B) {
    fn same(&self, o: &Self) -> bool {
        self.0.same(&o.0) && self.1.same(&o.1)
    }
}

/// Routes are equal when every waypoint and timing has the same bits.
pub fn same_route(a: &Route, b: &Route) -> bool {
    let bits = |r: &Route| -> Vec<u64> {
        r.waypoints.0.iter().flatten().chain(r.timings.0.iter()).map(|v| v.to_bits()).collect()
    };
    bits(a) == bits(b)
}

const PENALTY: f64 = 1000.0;
const HOVER_POWER: f64 = 1.0;
const DRAG: f64 = 0.2;

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn norm(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}

fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn wind(s: &Scenario, p: [f64; 2], t: f64) -> [f64; 2] {
    let w = &s.wind;
    let angle = TAU * t / w.period;
    let (sin, cos) = angle.sin_cos();
    let strength = 1.0 + w.ripple * (0.3 * p[0]).sin() * (0.3 * p[1]).cos();
    [(w.base[0] * cos - w.base[1] * sin) * strength, (w.base[0] * sin + w.base[1] * cos) * strength]
}

fn intrusion(a: [f64; 2], b: [f64; 2], zone: &Zone) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let t = if len2 > 0.0 { (dot(sub(zone.center, a), ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    (zone.radius - norm(sub(zone.center, closest))).max(0.0)
}

fn points(s: &Scenario, r: &Route) -> Vec<[f64; 2]> {
    std::iter::once(s.start).chain(r.waypoints.0.iter().copied()).chain(std::iter::once(s.goal)).collect()
}

fn min_leg_time(s: &Scenario, a: [f64; 2], b: [f64; 2], t: f64) -> f64 {
    let d = sub(b, a);
    let d2 = dot(d, d);
    if d2 < 1e-12 {
        return 0.0;
    }
    let w = wind(s, midpoint(a, b), t);
    let dw = dot(d, w);
    let disc = dw * dw - d2 * (dot(w, w) - s.max_airspeed * s.max_airspeed);
    let s_max = (dw + disc.max(0.0).sqrt()) / d2;
    1.0 / s_max
}

fn airspeed(s: &Scenario, a: [f64; 2], b: [f64; 2], t: f64, dt: f64) -> f64 {
    let d = sub(b, a);
    let w = wind(s, midpoint(a, b), t);
    norm([d[0] / dt - w[0], d[1] / dt - w[1]])
}

/// `Scenario::score`, computed by the verifier.
pub fn score(s: &Scenario, route: &Route) -> f64 {
    let points = points(s, route);
    let mut time = 0.0;
    let mut energy = 0.0;
    let mut violation = 0.0;

    for (i, &dt) in route.timings.0.iter().enumerate() {
        let (a, b) = (points[i], points[i + 1]);
        let min = min_leg_time(s, a, b, time);
        violation += (min - dt).max(0.0) + (dt - s.max_leg_time).max(0.0);

        let dt_used = dt.max(min).max(1e-9);
        let speed = airspeed(s, a, b, time, dt_used);
        energy += dt_used * (HOVER_POWER + DRAG * speed * speed * speed);
        time += dt_used;

        for zone in &s.zones {
            violation += intrusion(a, b, zone);
        }
    }

    for p in &route.waypoints.0 {
        for k in 0..2 {
            violation += (s.area[0][k] - p[k]).max(0.0) + (p[k] - s.area[1][k]).max(0.0);
        }
    }

    -(time + s.energy_weight * energy) - PENALTY * violation
}

/// `Scenario::check`, computed by the verifier.
pub fn check(s: &Scenario, route: &Route, tolerance: f64) -> Result<(), String> {
    let points = points(s, route);
    let mut time = 0.0;
    for (i, &dt) in route.timings.0.iter().enumerate() {
        let (a, b) = (points[i], points[i + 1]);
        let min = min_leg_time(s, a, b, time);
        if dt < min - tolerance {
            return Err(format!("leg {i} takes {dt}, faster than its minimum {min}"));
        }
        if dt > s.max_leg_time + tolerance {
            return Err(format!("leg {i} takes {dt}, longer than {}", s.max_leg_time));
        }
        for (z, zone) in s.zones.iter().enumerate() {
            let depth = intrusion(a, b, zone);
            if depth > tolerance {
                return Err(format!("leg {i} cuts {depth} into zone {z}"));
            }
        }
        time += dt;
    }
    for (i, p) in route.waypoints.0.iter().enumerate() {
        for k in 0..2 {
            if p[k] < s.area[0][k] - tolerance || p[k] > s.area[1][k] + tolerance {
                return Err(format!("waypoint {i} is outside the area: {p:?}"));
            }
        }
    }
    Ok(())
}
