//! Helpers for the hidden tests: recording components and a user-written mover.
#![allow(dead_code)]

use nestswarm::{Boundary, Contextful, FitCalc, GBest, Mover, NestedInit, Particle, ParticleMut, Unit};
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
pub struct RecFit<F: FitCalc> {
    pub inner: F,
    pub context: F::TContext,
    pub log: Log<F::TContext>,
}

impl<F: FitCalc> RecFit<F>
where
    F::TContext: Default,
{
    pub fn new(inner: F, log: Log<F::TContext>) -> Self {
        RecFit { inner, context: Default::default(), log }
    }
}

impl<F: FitCalc> Contextful for RecFit<F> {
    type TContext = F::TContext;
    fn set_context(&mut self, context: Self::TContext) {
        self.context = context;
        self.inner.set_context(context);
    }
}

impl<F: FitCalc> FitCalc for RecFit<F> {
    type T = F::T;
    fn calculate_fit(&self, position: Self::T) -> f64 {
        self.log.push(Event::Fit(self.context));
        self.inner.calculate_fit(position)
    }
}

/// Wraps a boundary and records the context at every call.
#[derive(Clone, Debug)]
pub struct RecBound<B: Boundary> {
    pub inner: B,
    pub context: B::TContext,
    pub log: Log<B::TContext>,
}

impl<B: Boundary> RecBound<B>
where
    B::TContext: Default,
{
    pub fn new(inner: B, log: Log<B::TContext>) -> Self {
        RecBound { inner, context: Default::default(), log }
    }
}

impl<B: Boundary> Contextful for RecBound<B> {
    type TContext = B::TContext;
    fn set_context(&mut self, context: Self::TContext) {
        self.context = context;
        self.inner.set_context(context);
    }
}

impl<B: Boundary> Boundary for RecBound<B> {
    type T = B::T;
    fn handle(&self, position: Self::T) -> Self::T {
        self.log.push(Event::Bound(self.context));
        self.inner.handle(position)
    }
}

/// Wraps a mover and records the context at every move.
#[derive(Clone, Debug)]
pub struct RecMover<M: Mover> {
    pub inner: M,
    pub context: M::TContext,
    pub log: Log<M::TContext>,
}

impl<M: Mover> RecMover<M>
where
    M::TContext: Default,
{
    pub fn new(inner: M, log: Log<M::TContext>) -> Self {
        RecMover { inner, context: Default::default(), log }
    }
}

impl<M: Mover> Contextful for RecMover<M> {
    type TContext = M::TContext;
    fn set_context(&mut self, context: Self::TContext) {
        self.context = context;
        self.inner.set_context(context);
    }
}

impl<M: Mover> Mover for RecMover<M> {
    type TUnit = M::TUnit;
    type TCommon = M::TCommon;
    fn update(&mut self, common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>) {
        self.log.push(Event::Move(self.context));
        self.inner.update(common, p);
    }
}

/// Wraps an initializer and records the outer particle of every inner search.
#[derive(Clone, Debug)]
pub struct RecInit<I: NestedInit> {
    pub inner: I,
    pub log: Log<I::TOuter>,
}

impl<I: NestedInit> NestedInit for RecInit<I> {
    type TOuter = I::TOuter;
    type TInner = I::TInner;
    fn init<R: Rng + ?Sized>(&self, outer: Self::TOuter, rng: &mut R) -> Vec<Particle<Self::TInner>> {
        self.log.push(Event::Init(outer));
        self.inner.init(outer, rng)
    }
}

/// A user-written PSO variant with a constriction factor, independent of the
/// library's own mover. Generic over unit and context.
#[derive(Clone, Debug)]
pub struct ConstrictedPso<U, R, C> {
    pub rng: R,
    pub phantom: std::marker::PhantomData<fn() -> (U, C)>,
}

impl<U, R, C> ConstrictedPso<U, R, C> {
    pub fn with_rng(rng: R) -> Self {
        ConstrictedPso { rng, phantom: std::marker::PhantomData }
    }
}

impl<U, R, C: Copy> Contextful for ConstrictedPso<U, R, C> {
    type TContext = C;
    fn set_context(&mut self, _: C) {}
}

impl<U: Unit, R: Rng, C: Copy> Mover for ConstrictedPso<U, R, C> {
    type TUnit = U;
    type TCommon = GBest<U>;
    fn update(&mut self, common: &GBest<U>, p: ParticleMut<'_, U>) {
        const CHI: f64 = 0.7298;
        const PHI: f64 = 2.05;
        let rng = &mut self.rng;
        let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * PHI * rng.random::<f64>());
        let social = (common.best_pos - *p.pos).map_scalars(|d| d * PHI * rng.random::<f64>());
        let v = *p.vel + cognitive + social;
        *p.vel = v * CHI;
        *p.pos = *p.pos + *p.vel;
    }
}

pub fn assert_send<S: Send>(_: &S) {}
pub fn assert_clone<S: Clone>(_: &S) {}
