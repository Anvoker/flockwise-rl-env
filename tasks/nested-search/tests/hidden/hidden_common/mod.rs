//! Helpers for the hidden tests: recording components and a user-written mover.
#![allow(dead_code)]

use flockwise::{Region, OuterAware, Objective, BestSoFar, Rule, Seeder, Particle, ParticleView, Point};
use rand::Rng;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

/// Something that happened inside a composition, with the context active at the time.
#[derive(Clone, Debug, PartialEq)]
pub enum Event<C> {
    /// An inner search was initialized for this outer particle.
    Init(C),
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
/// iterations), the fitness, boundary and mover must each have been called
/// exactly `m * n` times with that search's outer particle as context, and
/// never with any other context.
pub fn check_counts<C: Debug + Clone>(events: &[Event<C>], per_search: usize, kinds: &[&str]) {
    let mut expected: HashMap<String, usize> = HashMap::new();
    for e in events {
        if let Event::Init(c) = e {
            *expected.entry(key(c)).or_default() += per_search;
        }
    }
    assert!(!expected.is_empty(), "no inner search was initialized");

    for kind in kinds {
        let mut seen: HashMap<String, usize> = HashMap::new();
        for e in events {
            let c = match (e, *kind) {
                (Event::Fit(c), "fit") | (Event::Bound(c), "bound") | (Event::Move(c), "move") => c,
                _ => continue,
            };
            *seen.entry(key(c)).or_default() += 1;
        }
        for (ctx, n) in &seen {
            let want = expected.get(ctx).copied().unwrap_or(0);
            assert_eq!(*n, want, "{kind}: {n} calls with context {ctx}, expected {want}");
        }
        for (ctx, want) in &expected {
            let n = seen.get(ctx).copied().unwrap_or(0);
            assert_eq!(n, *want, "{kind}: {n} calls with context {ctx}, expected {want}");
        }
    }
}

/// Wraps a fitness function and records the context at every evaluation.
#[derive(Clone, Debug)]
pub struct RecFit<F: Objective> {
    pub inner: F,
    pub context: F::Outer,
    pub log: Log<F::Outer>,
}

impl<F: Objective> RecFit<F>
where
    F::Outer: Default,
{
    pub fn new(inner: F, log: Log<F::Outer>) -> Self {
        RecFit { inner, context: Default::default(), log }
    }
}

impl<F: Objective> OuterAware for RecFit<F> {
    type Outer = F::Outer;
    fn see_outer(&mut self, context: Self::Outer) {
        self.context = context;
        self.inner.see_outer(context);
    }
}

impl<F: Objective> Objective for RecFit<F> {
    type Pos = F::Pos;
    fn score(&self, position: Self::Pos) -> f64 {
        self.log.push(Event::Fit(self.context));
        self.inner.score(position)
    }
}

/// Wraps a boundary and records the context at every call.
#[derive(Clone, Debug)]
pub struct RecBound<B: Region> {
    pub inner: B,
    pub context: B::Outer,
    pub log: Log<B::Outer>,
}

impl<B: Region> RecBound<B>
where
    B::Outer: Default,
{
    pub fn new(inner: B, log: Log<B::Outer>) -> Self {
        RecBound { inner, context: Default::default(), log }
    }
}

impl<B: Region> OuterAware for RecBound<B> {
    type Outer = B::Outer;
    fn see_outer(&mut self, context: Self::Outer) {
        self.context = context;
        self.inner.see_outer(context);
    }
}

impl<B: Region> Region for RecBound<B> {
    type Pos = B::Pos;
    fn confine(&self, position: Self::Pos) -> Self::Pos {
        self.log.push(Event::Bound(self.context));
        self.inner.confine(position)
    }
}

/// Wraps a mover and records the context at every move.
#[derive(Clone, Debug)]
pub struct RecMover<M: Rule> {
    pub inner: M,
    pub context: M::Outer,
    pub log: Log<M::Outer>,
}

impl<M: Rule> RecMover<M>
where
    M::Outer: Default,
{
    pub fn new(inner: M, log: Log<M::Outer>) -> Self {
        RecMover { inner, context: Default::default(), log }
    }
}

impl<M: Rule> OuterAware for RecMover<M> {
    type Outer = M::Outer;
    fn see_outer(&mut self, context: Self::Outer) {
        self.context = context;
        self.inner.see_outer(context);
    }
}

impl<M: Rule> Rule for RecMover<M> {
    type Pos = M::Pos;
    type Shared = M::Shared;
    fn apply<R: Rng + ?Sized>(&self, common: &Self::Shared, rng: &mut R, p: ParticleView<'_, Self::Pos>) {
        self.log.push(Event::Move(self.context));
        self.inner.apply(common, rng, p);
    }
}

/// Wraps an initializer and records the outer particle of every inner search.
#[derive(Clone, Debug)]
pub struct RecInit<I: Seeder> {
    pub inner: I,
    pub log: Log<I::Outer>,
}

impl<I: Seeder> Seeder for RecInit<I> {
    type Outer = I::Outer;
    type Inner = I::Inner;
    fn seed<R: Rng + ?Sized>(&self, outer: Self::Outer, rng: &mut R) -> Vec<Particle<Self::Inner>> {
        self.log.push(Event::Init(outer));
        self.inner.seed(outer, rng)
    }
}

/// A user-written PSO variant with a constriction factor, independent of the
/// library's own rule. Generic over unit and outer position.
#[derive(Clone, Debug)]
pub struct ConstrictedPso<U, C> {
    pub phantom: std::marker::PhantomData<fn() -> (U, C)>,
}

impl<U, C> ConstrictedPso<U, C> {
    pub fn new() -> Self {
        ConstrictedPso { phantom: std::marker::PhantomData }
    }
}

impl<U, C: Copy + Send + Sync> OuterAware for ConstrictedPso<U, C> {
    type Outer = C;
    fn see_outer(&mut self, _: C) {}
}

impl<U: Point, C: Copy + Send + Sync> Rule for ConstrictedPso<U, C> {
    type Pos = U;
    type Shared = BestSoFar<U>;
    fn apply<R: Rng + ?Sized>(&self, common: &BestSoFar<U>, rng: &mut R, p: ParticleView<'_, U>) {
        const CHI: f64 = 0.7298;
        const PHI: f64 = 2.05;
        let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * PHI * rng.random::<f64>());
        let social = (common.best_pos - *p.pos).map_scalars(|d| d * PHI * rng.random::<f64>());
        let v = *p.vel + cognitive + social;
        *p.vel = v * CHI;
        *p.pos = *p.pos + *p.vel;
    }
}

pub fn assert_send<S: Send>(_: &S) {}
pub fn assert_clone<S: Clone>(_: &S) {}
