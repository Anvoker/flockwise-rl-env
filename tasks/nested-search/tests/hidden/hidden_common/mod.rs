//! Helpers for the hidden tests: recording closures and a user-written rule.
#![allow(dead_code)]

use flockwise::{BestSoFar, Particle, ParticleView, Point};
use rand::{Rng, RngCore};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

/// Something that happened inside a composition, with the outer position
/// the component was prepared for.
#[derive(Clone, Debug, PartialEq)]
pub enum Event<C> {
    /// An inner search was seeded for this outer particle.
    Init(C),
    /// A component's closure was called with this outer position.
    Prep(&'static str, C),
    Fit(C),
    Bound(C),
    Move(C),
}

#[derive(Clone, Debug)]
pub struct Log<C>(pub Arc<Mutex<Vec<Event<C>>>>);

impl<C: Clone> Log<C> {
    pub fn new() -> Self {
        Log(Arc::new(Mutex::new(Vec::new())))
    }

    pub fn push(&self, e: Event<C>) {
        self.0.lock().unwrap().push(e);
    }

    pub fn events(&self) -> Vec<Event<C>> {
        self.0.lock().unwrap().clone()
    }
}

/// A position as an exact, hashable key.
pub fn key<C: Debug>(c: &C) -> String {
    format!("{c:?}")
}

/// For every inner search (one `Init` per search, with `m` particles and `n`
/// iterations), each of `kinds` must have been prepared exactly once for
/// that search's outer particle, and called exactly `m * n` times with it,
/// and never with any other outer position.
pub fn check_counts<C: Debug + Clone>(events: &[Event<C>], per_search: usize, kinds: &[&str]) {
    let mut searches: HashMap<String, usize> = HashMap::new();
    for e in events {
        if let Event::Init(c) = e {
            *searches.entry(key(c)).or_default() += 1;
        }
    }
    assert!(!searches.is_empty(), "no inner search was seeded");

    for kind in kinds {
        let mut preps: HashMap<String, usize> = HashMap::new();
        let mut calls: HashMap<String, usize> = HashMap::new();
        for e in events {
            match (e, *kind) {
                (Event::Prep(k, c), _) if k == kind => *preps.entry(key(c)).or_default() += 1,
                (Event::Fit(c), "fit") | (Event::Bound(c), "bound") | (Event::Move(c), "move") => {
                    *calls.entry(key(c)).or_default() += 1
                }
                _ => {}
            }
        }
        for (ctx, n) in preps.iter().chain(calls.iter()) {
            assert!(searches.contains_key(ctx), "{kind}: {n} preparations or calls for {ctx}, which no inner search ran for");
        }
        for (ctx, count) in &searches {
            let p = preps.get(ctx).copied().unwrap_or(0);
            assert_eq!(p, *count, "{kind}: prepared {p} times for {ctx}, expected once per search ({count})");
            let n = calls.get(ctx).copied().unwrap_or(0);
            assert_eq!(n, count * per_search, "{kind}: {n} calls for {ctx}, expected {}", count * per_search);
        }
    }
}

/// For every outer position, each kind's first event is its preparation:
/// no component is called for an inner search before it was prepared for it.
pub fn check_prepared_first<C: Debug + Clone>(events: &[Event<C>]) {
    let mut prepared: HashMap<(String, &str), usize> = HashMap::new();
    for (i, e) in events.iter().enumerate() {
        let (kind, c) = match e {
            Event::Prep(k, c) => {
                prepared.entry((key(c), *k)).or_insert(i);
                continue;
            }
            Event::Fit(c) => ("fit", c),
            Event::Bound(c) => ("bound", c),
            Event::Move(c) => ("move", c),
            Event::Init(_) => continue,
        };
        assert!(
            prepared.contains_key(&(key(c), kind)),
            "{kind} was called for {} before it was prepared for it",
            key(c)
        );
    }
}

// Recording wrappers. Each is called from a component's closure with the
// outer position it was given, records that, and returns a closure that
// records the same outer position at every call:
// `objective(move |w: W| rec_fit(log.clone(), w, inner_fit(problem, w)))`.

pub fn rec_fit<C, P>(log: Log<C>, outer: C, f: impl Fn(P) -> f64 + Clone + Send + Sync) -> impl Fn(P) -> f64 + Clone + Send + Sync
where
    C: Copy + Send + Sync + 'static,
{
    log.push(Event::Prep("fit", outer));
    move |pos: P| {
        log.push(Event::Fit(outer));
        f(pos)
    }
}

pub fn rec_bound<C, P>(log: Log<C>, outer: C, f: impl Fn(P) -> P + Clone + Send + Sync) -> impl Fn(P) -> P + Clone + Send + Sync
where
    C: Copy + Send + Sync + 'static,
{
    log.push(Event::Prep("bound", outer));
    move |pos: P| {
        log.push(Event::Bound(outer));
        f(pos)
    }
}

pub fn rec_move<C, P>(
    log: Log<C>,
    outer: C,
    f: impl Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Clone + Send + Sync,
) -> impl Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>) + Clone + Send + Sync
where
    C: Copy + Send + Sync + 'static,
{
    log.push(Event::Prep("move", outer));
    move |best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>| {
        log.push(Event::Move(outer));
        f(best, rng, p)
    }
}

/// Wraps a seeder's closure and records the outer particle of every inner search.
pub fn rec_init<C, I>(
    log: Log<C>,
    f: impl Fn(C, &mut dyn RngCore) -> Vec<Particle<I>> + Clone + Send + Sync,
) -> impl Fn(C, &mut dyn RngCore) -> Vec<Particle<I>> + Clone + Send + Sync
where
    C: Copy + Send + Sync + 'static,
{
    move |outer: C, rng: &mut dyn RngCore| {
        log.push(Event::Init(outer));
        f(outer, rng)
    }
}

/// A user-written PSO variant with a constriction factor, independent of the
/// library's own rule.
pub fn constricted_pso<U: Point>() -> impl Fn(&BestSoFar<U>, &mut dyn RngCore, ParticleView<'_, U>) + Clone + Send + Sync {
    |best: &BestSoFar<U>, rng: &mut dyn RngCore, p: ParticleView<'_, U>| {
        const CHI: f64 = 0.7298;
        const PHI: f64 = 2.05;
        let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * PHI * rng.random::<f64>());
        let social = (best.best_pos - *p.pos).map_scalars(|d| d * PHI * rng.random::<f64>());
        let v = *p.vel + cognitive + social;
        *p.vel = v * CHI;
        *p.pos = *p.pos + *p.vel;
    }
}

pub fn assert_send<S: Send>(_: &S) {}
pub fn assert_clone<S: Clone>(_: &S) {}
