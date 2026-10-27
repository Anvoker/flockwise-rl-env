use crate::api::{BestSoFar, ParticleView, Point, ReadFrom, SplitFrom, WriteInto};
use crate::common::{PreparedRule, Region, Rule, Seeder};
use crate::rng::seeder_stream;
use crate::swarm::{PreparedSearch, Search};
use rand::{Rng, RngCore};
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PsoParams {
    pub inertia: f64,
    pub cognitive: f64,
    pub social: f64,
}

/// Standard global-best particle swarm update. Works for any position and
/// any outer position.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PsoRule {
    params: PsoParams,
}

impl PsoRule {
    pub fn new(params: PsoParams) -> Self {
        PsoRule { params }
    }
}

impl<P: Point, O> Rule<P, O> for PsoRule {
    type Prepared<'a>
        = PsoPrepared
    where
        Self: 'a;

    fn prepare<'a>(&'a self, _outer: O) -> PsoPrepared {
        PsoPrepared { params: self.params }
    }
}

pub struct PsoPrepared {
    params: PsoParams,
}

impl<P: Point> PreparedRule<P> for PsoPrepared {
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        let PsoParams { inertia, cognitive, social } = self.params;
        let x = *p.pos;
        let own = (*p.best_pos - x).map_scalars(|d| cognitive * rng.random::<f64>() * d);
        let global = (best.best_pos - x).map_scalars(|d| social * rng.random::<f64>() * d);
        let vel = *p.vel * inertia + own + global;
        *p.vel = vel;
        *p.pos = x + vel;
    }
}

/// Applies a region to the position after a rule has moved it.
#[derive(Clone)]
pub struct Confined<R, G> {
    rule: R,
    region: G,
}

impl<R, G> Confined<R, G> {
    pub fn new(rule: R, region: G) -> Self {
        Confined { rule, region }
    }
}

pub struct ConfinedPrepared<A, G> {
    rule: A,
    region: G,
}

impl<P, O, R, G> Rule<P, O> for Confined<R, G>
where
    P: Point,
    O: Copy,
    R: Rule<P, O>,
    G: Region<Pos = P, Outer = O>,
{
    type Prepared<'a>
        = ConfinedPrepared<<R as Rule<P, O>>::Prepared<'a>, G::Prepared<'a>>
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: O) -> Self::Prepared<'a> {
        ConfinedPrepared { rule: self.rule.prepare(outer), region: self.region.prepare(outer) }
    }
}

impl<P, A, G> PreparedRule<P> for ConfinedPrepared<A, G>
where
    P: Point,
    A: PreparedRule<P>,
    G: Fn(P) -> P + Sync,
{
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, mut p: ParticleView<'_, P>) {
        self.rule.apply(best, rng, p.reborrow());
        *p.pos = (self.region)(*p.pos);
    }
}

/// Runs a rule on one field of a larger position.
pub struct FieldRule<R, F> {
    rule: R,
    _field: PhantomData<fn() -> F>,
}

impl<R: Clone, F> Clone for FieldRule<R, F> {
    fn clone(&self) -> Self {
        FieldRule { rule: self.rule.clone(), _field: PhantomData }
    }
}

pub struct FieldPrepared<A, F> {
    rule: A,
    _field: PhantomData<fn() -> F>,
}

/// Provides `for_field` on rules.
pub trait ForField: Sized {
    fn for_field<F>(self) -> FieldRule<Self, F> {
        FieldRule { rule: self, _field: PhantomData }
    }
}

impl<T> ForField for T {}

impl<W, O, R, F> Rule<W, O> for FieldRule<R, F>
where
    W: Point,
    F: SplitFrom<Whole = W> + ReadFrom<Whole = W>,
    R: Rule<F, O>,
{
    type Prepared<'a>
        = FieldPrepared<<R as Rule<F, O>>::Prepared<'a>, F>
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: O) -> Self::Prepared<'a> {
        FieldPrepared { rule: self.rule.prepare(outer), _field: PhantomData }
    }
}

impl<W, F, A> PreparedRule<W> for FieldPrepared<A, F>
where
    W: Point,
    F: SplitFrom<Whole = W> + ReadFrom<Whole = W>,
    A: PreparedRule<F>,
{
    fn apply(&self, best: &BestSoFar<W>, rng: &mut dyn RngCore, p: ParticleView<'_, W>) {
        let field_best = BestSoFar { best_pos: F::read(&best.best_pos), best_score: best.best_score };
        self.rule.apply(&field_best, rng, <F as SplitFrom>::split(p));
    }
}

/// A rule that runs a whole inner search for every particle it moves, and
/// writes the best position found into the particle's field.
#[derive(Clone)]
pub struct InnerSearch<S, D> {
    iterations: usize,
    search: S,
    seeder: D,
}

impl<S, D> InnerSearch<S, D> {
    pub fn new(iterations: usize, search: S, seeder: D) -> Self {
        InnerSearch { iterations, search, seeder }
    }
}

pub struct InnerPrepared<'a, S, D> {
    iterations: usize,
    search: &'a S,
    seeder: &'a D,
}

impl<W, O, S, D> Rule<W, O> for InnerSearch<S, D>
where
    W: Point,
    S: Search<Outer = W>,
    S::Pos: WriteInto<Whole = W>,
    D: Seeder<Outer = W, Inner = S::Pos>,
{
    type Prepared<'a>
        = InnerPrepared<'a, S, D>
    where
        Self: 'a;

    fn prepare<'a>(&'a self, _outer: O) -> InnerPrepared<'a, S, D> {
        InnerPrepared { iterations: self.iterations, search: &self.search, seeder: &self.seeder }
    }
}

impl<W, S, D> PreparedRule<W> for InnerPrepared<'_, S, D>
where
    W: Point,
    S: Search<Outer = W>,
    S::Pos: WriteInto<Whole = W>,
    D: Seeder<Outer = W, Inner = S::Pos>,
{
    fn apply(&self, _best: &BestSoFar<W>, rng: &mut dyn RngCore, p: ParticleView<'_, W>) {
        // Everything in the inner search derives from this one number, drawn
        // from the rule's own stream.
        let seed = rng.next_u64();
        let outer = *p.pos;

        let mut particles = self.seeder.seed(outer, &mut seeder_stream(seed));
        let prepared = self.search.prepare(outer);
        let mut best = BestSoFar::new();
        for iteration in 0..self.iterations {
            prepared.step(seed, iteration as u64, &mut best, &mut particles);
        }
        best.best_pos.write(p.pos);
    }
}
