//! The drone delivery model: routes, wind, no-fly zones, flight physics, and
//! the objectives, regions and seeders a planner searches with.
//!
//! This file is part of the public contract and must not be changed.
//!
//! A route flies from `start` to `goal` through `STOPS` waypoints, so it has
//! `LEGS` legs. Each leg has a duration. The wind depends on where and when a
//! leg is flown, so a leg's fastest possible duration depends on the
//! waypoints and on how long the earlier legs took.

use crate::api::{
    Objective, OuterAware, Particle, ParticleView, Point, ReadFrom, Region, Seeder, SplitFrom,
    WriteInto,
};
use rand::Rng;
use std::f64::consts::TAU;
use std::ops::{Add, Mul, Sub};

pub const STOPS: usize = 5;
pub const LEGS: usize = STOPS + 1;
pub const ZONES: usize = 3;

/// Score lost per unit of constraint violation.
const PENALTY: f64 = 1000.0;
/// Power drawn while flying, besides drag.
const HOVER_POWER: f64 = 1.0;
const DRAG: f64 = 0.2;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Waypoints(pub [[f64; 2]; STOPS]);

/// Leg durations.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Timings(pub [f64; LEGS]);

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Route {
    pub waypoints: Waypoints,
    pub timings: Timings,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Zone {
    pub center: [f64; 2],
    pub radius: f64,
}

/// Wind turns around over time and is stronger in some places than others:
/// `rotate(base, TAU * t / period) * (1 + ripple * sin(0.3 x) * cos(0.3 y))`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Wind {
    pub base: [f64; 2],
    pub period: f64,
    pub ripple: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Scenario {
    pub start: [f64; 2],
    pub goal: [f64; 2],
    /// Waypoints must stay inside this box: `[lo, hi]`.
    pub area: [[f64; 2]; 2],
    pub zones: [Zone; ZONES],
    pub wind: Wind,
    /// Airspeed can't exceed this. It is higher than any wind.
    pub max_airspeed: f64,
    pub max_leg_time: f64,
    /// How much energy counts against time in the score.
    pub energy_weight: f64,
}

impl Wind {
    pub fn at(&self, p: [f64; 2], t: f64) -> [f64; 2] {
        let angle = TAU * t / self.period;
        let (s, c) = angle.sin_cos();
        let strength = 1.0 + self.ripple * (0.3 * p[0]).sin() * (0.3 * p[1]).cos();
        [
            (self.base[0] * c - self.base[1] * s) * strength,
            (self.base[0] * s + self.base[1] * c) * strength,
        ]
    }
}

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

/// How far a segment cuts into a zone: 0 if it stays outside.
fn intrusion(a: [f64; 2], b: [f64; 2], zone: &Zone) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let t = if len2 > 0.0 { (dot(sub(zone.center, a), ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    (zone.radius - norm(sub(zone.center, closest))).max(0.0)
}

impl Scenario {
    /// The points a route visits, start and goal included.
    pub fn points(&self, waypoints: &Waypoints) -> [[f64; 2]; STOPS + 2] {
        let mut points = [self.start; STOPS + 2];
        points[1..=STOPS].copy_from_slice(&waypoints.0);
        points[STOPS + 1] = self.goal;
        points
    }

    /// The shortest duration for flying from `a` to `b` starting at time
    /// `t`, given the wind at the leg's midpoint and the airspeed limit.
    pub fn min_leg_time(&self, a: [f64; 2], b: [f64; 2], t: f64) -> f64 {
        let d = sub(b, a);
        let d2 = dot(d, d);
        if d2 < 1e-12 {
            return 0.0;
        }
        // Ground velocity is d / dt, airspeed is |d / dt - w|. With s = 1 / dt,
        // |s d - w| <= v_max holds for s up to the larger root of
        // s^2 |d|^2 - 2 s (d . w) + |w|^2 - v_max^2 = 0.
        let w = self.wind.at(midpoint(a, b), t);
        let dw = dot(d, w);
        let disc = dw * dw - d2 * (dot(w, w) - self.max_airspeed * self.max_airspeed);
        let s_max = (dw + disc.max(0.0).sqrt()) / d2;
        1.0 / s_max
    }

    /// Airspeed on a leg from `a` to `b` that starts at `t` and takes `dt`.
    pub fn airspeed(&self, a: [f64; 2], b: [f64; 2], t: f64, dt: f64) -> f64 {
        let d = sub(b, a);
        let w = self.wind.at(midpoint(a, b), t);
        norm([d[0] / dt - w[0], d[1] / dt - w[1]])
    }

    /// The score of a route: minus (total time + energy_weight * energy), with
    /// a large penalty per unit of any constraint violation. Higher is better.
    pub fn score(&self, route: &Route) -> f64 {
        let points = self.points(&route.waypoints);
        let mut time = 0.0;
        let mut energy = 0.0;
        let mut violation = 0.0;

        for (i, &dt) in route.timings.0.iter().enumerate() {
            let (a, b) = (points[i], points[i + 1]);
            let min = self.min_leg_time(a, b, time);
            violation += (min - dt).max(0.0) + (dt - self.max_leg_time).max(0.0);

            let dt_used = dt.max(min).max(1e-9);
            let airspeed = self.airspeed(a, b, time, dt_used);
            energy += dt_used * (HOVER_POWER + DRAG * airspeed * airspeed * airspeed);
            time += dt_used;

            for zone in &self.zones {
                violation += intrusion(a, b, zone);
            }
        }

        for p in &route.waypoints.0 {
            for k in 0..2 {
                violation += (self.area[0][k] - p[k]).max(0.0) + (p[k] - self.area[1][k]).max(0.0);
            }
        }

        -(time + self.energy_weight * energy) - PENALTY * violation
    }

    /// Why a route is infeasible, if it is. Allows `tolerance` of slack.
    pub fn check(&self, route: &Route, tolerance: f64) -> Result<(), String> {
        let points = self.points(&route.waypoints);
        let mut time = 0.0;
        for (i, &dt) in route.timings.0.iter().enumerate() {
            let (a, b) = (points[i], points[i + 1]);
            let min = self.min_leg_time(a, b, time);
            if dt < min - tolerance {
                return Err(format!("leg {i} takes {dt}, faster than its minimum {min}"));
            }
            if dt > self.max_leg_time + tolerance {
                return Err(format!("leg {i} takes {dt}, longer than {}", self.max_leg_time));
            }
            for (z, zone) in self.zones.iter().enumerate() {
                let depth = intrusion(a, b, zone);
                if depth > tolerance {
                    return Err(format!("leg {i} cuts {depth} into zone {z}"));
                }
            }
            time += dt;
        }
        for (i, p) in route.waypoints.0.iter().enumerate() {
            for k in 0..2 {
                if p[k] < self.area[0][k] - tolerance || p[k] > self.area[1][k] + tolerance {
                    return Err(format!("waypoint {i} is outside the area: {p:?}"));
                }
            }
        }
        Ok(())
    }

    /// Fastest feasible timings for the given waypoints, leg by leg.
    pub fn fastest_timings(&self, waypoints: &Waypoints) -> Timings {
        let points = self.points(waypoints);
        let mut timings = [0.0; LEGS];
        let mut time = 0.0;
        for i in 0..LEGS {
            timings[i] = self.min_leg_time(points[i], points[i + 1], time).min(self.max_leg_time);
            time += timings[i];
        }
        Timings(timings)
    }

    /// The straight route, flown as fast as possible. A planner should do
    /// much better whenever a zone or the wind is in the way.
    pub fn baseline(&self) -> Route {
        let mut waypoints = [[0.0; 2]; STOPS];
        for (i, w) in waypoints.iter_mut().enumerate() {
            let f = (i + 1) as f64 / (STOPS + 1) as f64;
            *w = [
                self.start[0] + (self.goal[0] - self.start[0]) * f,
                self.start[1] + (self.goal[1] - self.start[1]) * f,
            ];
        }
        let waypoints = Waypoints(waypoints);
        Route {
            waypoints,
            timings: self.fastest_timings(&waypoints),
        }
    }

    /// Starting routes: bowed to either side of the straight line by random
    /// amounts, flown at random feasible speeds.
    pub fn initial_routes<R: Rng + ?Sized>(&self, count: usize, rng: &mut R) -> Vec<Particle<Route>> {
        let straight = self.baseline().waypoints;
        let d = sub(self.goal, self.start);
        let length = norm(d);
        let normal = [-d[1] / length, d[0] / length];
        (0..count)
            .map(|_| {
                let bow = rng.random_range(-0.6..=0.6) * length;
                let mut waypoints = straight;
                for (i, w) in waypoints.0.iter_mut().enumerate() {
                    let f = (i + 1) as f64 / (STOPS + 1) as f64;
                    let offset = bow * 4.0 * f * (1.0 - f);
                    *w = [w[0] + normal[0] * offset, w[1] + normal[1] * offset];
                }
                let waypoints = AirspaceRegion::new(*self).confine(waypoints);
                let timings = self.random_timings(&waypoints, rng);
                let vel = Waypoints(std::array::from_fn(|_| {
                    [rng.random_range(-1.0..=1.0), rng.random_range(-1.0..=1.0)]
                }));
                Particle::new(Route { waypoints, timings }, Route { waypoints: vel, timings: Timings::default() })
            })
            .collect()
    }

    fn random_timings<R: Rng + ?Sized>(&self, waypoints: &Waypoints, rng: &mut R) -> Timings {
        let points = self.points(waypoints);
        let mut timings = [0.0; LEGS];
        let mut time = 0.0;
        for i in 0..LEGS {
            let min = self.min_leg_time(points[i], points[i + 1], time);
            timings[i] = (min * rng.random_range(1.0..=1.6)).min(self.max_leg_time);
            time += timings[i];
        }
        Timings(timings)
    }
}

/// Scores whole routes. Takes no outer position.
#[derive(Copy, Clone, Debug)]
pub struct RouteObjective {
    scenario: Scenario,
}

impl RouteObjective {
    pub fn new(scenario: Scenario) -> Self {
        RouteObjective { scenario }
    }
}

impl OuterAware for RouteObjective {
    type Outer = ();
    fn see_outer(&mut self, _: ()) {}
}

impl Objective for RouteObjective {
    type Pos = Route;
    fn score(&self, route: Route) -> f64 {
        self.scenario.score(&route)
    }
}

/// Scores timings for the outer route's waypoints.
#[derive(Copy, Clone, Debug)]
pub struct TimingObjective {
    scenario: Scenario,
    route: Route,
}

impl TimingObjective {
    pub fn new(scenario: Scenario) -> Self {
        TimingObjective { scenario, route: Route::default() }
    }
}

impl OuterAware for TimingObjective {
    type Outer = Route;
    fn see_outer(&mut self, route: Route) {
        self.route = route;
    }
}

impl Objective for TimingObjective {
    type Pos = Timings;
    fn score(&self, timings: Timings) -> f64 {
        self.scenario.score(&Route { waypoints: self.route.waypoints, timings })
    }
}

/// Keeps waypoints inside the area and out of the zones. Takes no outer position.
#[derive(Copy, Clone, Debug)]
pub struct AirspaceRegion {
    scenario: Scenario,
}

impl AirspaceRegion {
    pub fn new(scenario: Scenario) -> Self {
        AirspaceRegion { scenario }
    }
}

impl OuterAware for AirspaceRegion {
    type Outer = ();
    fn see_outer(&mut self, _: ()) {}
}

impl Region for AirspaceRegion {
    type Pos = Waypoints;
    fn confine(&self, waypoints: Waypoints) -> Waypoints {
        let s = &self.scenario;
        Waypoints(waypoints.0.map(|mut p| {
            for k in 0..2 {
                p[k] = p[k].clamp(s.area[0][k], s.area[1][k]);
            }
            for zone in &s.zones {
                let d = sub(p, zone.center);
                let dist = norm(d);
                if dist < zone.radius {
                    let scale = zone.radius * 1.0001 / dist.max(1e-9);
                    p = [zone.center[0] + d[0] * scale, zone.center[1] + d[1] * scale];
                }
            }
            p
        }))
    }
}

/// Keeps each leg's duration between its fastest possible and the maximum,
/// for the outer route's waypoints. Each leg's fastest duration depends on
/// when it starts, so legs are confined in order.
#[derive(Copy, Clone, Debug)]
pub struct LegTimeRegion {
    scenario: Scenario,
    route: Route,
}

impl LegTimeRegion {
    pub fn new(scenario: Scenario) -> Self {
        LegTimeRegion { scenario, route: Route::default() }
    }
}

impl OuterAware for LegTimeRegion {
    type Outer = Route;
    fn see_outer(&mut self, route: Route) {
        self.route = route;
    }
}

impl Region for LegTimeRegion {
    type Pos = Timings;
    fn confine(&self, timings: Timings) -> Timings {
        let s = &self.scenario;
        let points = s.points(&self.route.waypoints);
        let mut out = timings.0;
        let mut time = 0.0;
        for i in 0..LEGS {
            let min = s.min_leg_time(points[i], points[i + 1], time);
            out[i] = out[i].clamp(min.min(s.max_leg_time), s.max_leg_time);
            time += out[i];
        }
        Timings(out)
    }
}

/// Seeds an inner search over timings: random feasible speeds for the outer
/// route's waypoints.
#[derive(Copy, Clone, Debug)]
pub struct TimingSeeder {
    scenario: Scenario,
    count: usize,
}

impl TimingSeeder {
    pub fn new(scenario: Scenario, count: usize) -> Self {
        TimingSeeder { scenario, count }
    }
}

impl Seeder for TimingSeeder {
    type Outer = Route;
    type Inner = Timings;
    fn seed<R: Rng + ?Sized>(&self, route: Route, rng: &mut R) -> Vec<Particle<Timings>> {
        (0..self.count)
            .map(|_| {
                let timings = self.scenario.random_timings(&route.waypoints, rng);
                Particle::new(timings, Timings::default())
            })
            .collect()
    }
}

// Arithmetic and field access for the route types.

impl Add for Waypoints {
    type Output = Self;
    fn add(self, r: Self) -> Self {
        Waypoints(std::array::from_fn(|i| [self.0[i][0] + r.0[i][0], self.0[i][1] + r.0[i][1]]))
    }
}

impl Sub for Waypoints {
    type Output = Self;
    fn sub(self, r: Self) -> Self {
        Waypoints(std::array::from_fn(|i| [self.0[i][0] - r.0[i][0], self.0[i][1] - r.0[i][1]]))
    }
}

impl Mul<f64> for Waypoints {
    type Output = Self;
    fn mul(self, r: f64) -> Self {
        Waypoints(self.0.map(|p| [p[0] * r, p[1] * r]))
    }
}

impl Point for Waypoints {
    fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
        Waypoints(self.0.map(|p| [f(p[0]), f(p[1])]))
    }
}

impl Add for Timings {
    type Output = Self;
    fn add(self, r: Self) -> Self {
        Timings(std::array::from_fn(|i| self.0[i] + r.0[i]))
    }
}

impl Sub for Timings {
    type Output = Self;
    fn sub(self, r: Self) -> Self {
        Timings(std::array::from_fn(|i| self.0[i] - r.0[i]))
    }
}

impl Mul<f64> for Timings {
    type Output = Self;
    fn mul(self, r: f64) -> Self {
        Timings(self.0.map(|t| t * r))
    }
}

impl Point for Timings {
    fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
        Timings(self.0.map(|t| f(t)))
    }
}

impl Add for Route {
    type Output = Self;
    fn add(self, r: Self) -> Self {
        Route { waypoints: self.waypoints + r.waypoints, timings: self.timings + r.timings }
    }
}

impl Sub for Route {
    type Output = Self;
    fn sub(self, r: Self) -> Self {
        Route { waypoints: self.waypoints - r.waypoints, timings: self.timings - r.timings }
    }
}

impl Mul<f64> for Route {
    type Output = Self;
    fn mul(self, r: f64) -> Self {
        Route { waypoints: self.waypoints * r, timings: self.timings * r }
    }
}

impl Point for Route {
    fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
        Route { waypoints: self.waypoints.map_scalars(&mut f), timings: self.timings.map_scalars(&mut f) }
    }
}

impl SplitFrom for Waypoints {
    type Whole = Route;
    fn split(whole: ParticleView<'_, Route>) -> ParticleView<'_, Waypoints> {
        ParticleView {
            pos: &mut whole.pos.waypoints,
            vel: &mut whole.vel.waypoints,
            score: whole.score,
            best_pos: &mut whole.best_pos.waypoints,
            best_score: whole.best_score,
        }
    }
}

impl ReadFrom for Waypoints {
    type Whole = Route;
    fn read(whole: &Route) -> Waypoints {
        whole.waypoints
    }
}

impl WriteInto for Waypoints {
    type Whole = Route;
    fn write(self, whole: &mut Route) {
        whole.waypoints = self;
    }
}

impl SplitFrom for Timings {
    type Whole = Route;
    fn split(whole: ParticleView<'_, Route>) -> ParticleView<'_, Timings> {
        ParticleView {
            pos: &mut whole.pos.timings,
            vel: &mut whole.vel.timings,
            score: whole.score,
            best_pos: &mut whole.best_pos.timings,
            best_score: whole.best_score,
        }
    }
}

impl ReadFrom for Timings {
    type Whole = Route;
    fn read(whole: &Route) -> Timings {
        whole.timings
    }
}

impl WriteInto for Timings {
    type Whole = Route;
    fn write(self, whole: &mut Route) {
        whole.timings = self;
    }
}
