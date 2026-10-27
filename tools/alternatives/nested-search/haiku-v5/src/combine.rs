//! Rules built from other parts: confining a rule to a region, running a rule
//! on one field, and nesting a whole search inside a rule.

use crate::api::{BestSoFar, ParticleView, Point, ReadFrom, SplitFrom, WriteInto};
use crate::common::{Region, Rule, Search, Seeder, run_search, stream};
use rand::RngCore;
use std::marker::PhantomData;

/// A rule whose result is then confined by a region.
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

impl<R, G> Rule for Confined<R, G>
where
    R: Rule,
    G: Region<Pos = R::Pos, Outer = R::Outer>,
{
    type Pos = R::Pos;
    type Outer = R::Outer;
    type Prepared = (R::Prepared, G::Prepared);

    fn prepare(&self, outer: R::Outer) -> Self::Prepared {
        (self.rule.prepare(outer), self.region.prepare(outer))
    }

    fn apply(&self, prepared: &Self::Prepared, best: &BestSoFar<R::Pos>, rng: &mut dyn RngCore, mut p: ParticleView<'_, R::Pos>) {
        self.rule.apply(&prepared.0, best, rng, p.reborrow());
        *p.pos = (prepared.1)(*p.pos);
    }
}

/// A rule on one field of a whole position.
#[derive(Clone)]
pub struct ForField<R, F> {
    rule: R,
    marker: PhantomData<fn() -> F>,
}

impl<R, F> ForField<R, F> {
    pub(crate) fn new(rule: R) -> Self {
        ForField { rule, marker: PhantomData }
    }
}

impl<R, F> Rule for ForField<R, F>
where
    R: Rule<Pos = F>,
    F: SplitFrom + ReadFrom<Whole = <F as SplitFrom>::Whole>,
{
    type Pos = <F as SplitFrom>::Whole;
    type Outer = R::Outer;
    type Prepared = R::Prepared;

    fn prepare(&self, outer: R::Outer) -> R::Prepared {
        self.rule.prepare(outer)
    }

    fn apply(
        &self,
        prepared: &R::Prepared,
        best: &BestSoFar<<F as SplitFrom>::Whole>,
        rng: &mut dyn RngCore,
        p: ParticleView<'_, <F as SplitFrom>::Whole>,
    ) {
        let field_best = BestSoFar {
            best_pos: F::read(&best.best_pos),
            best_score: best.best_score,
        };
        self.rule.apply(prepared, &field_best, rng, F::split(p));
    }
}

/// A rule that runs a whole inner search for each particle it moves. The inner
/// search's outer position is the particle's position, and the inner search's
/// best position is written into the particle's field of that type.
#[derive(Clone)]
pub struct InnerSearch<O, I, Sd> {
    iterations: usize,
    inner: I,
    seeder: Sd,
    marker: PhantomData<fn(O)>,
}

impl<O, I, Sd> InnerSearch<O, I, Sd> {
    pub fn new(iterations: usize, inner: I, seeder: Sd) -> Self {
        InnerSearch {
            iterations,
            inner,
            seeder,
            marker: PhantomData,
        }
    }
}

impl<O, I, Sd> Rule for InnerSearch<O, I, Sd>
where
    O: Copy,
    I: Search,
    I::Outer: Point,
    I::Pos: ReadFrom<Whole = I::Outer> + WriteInto<Whole = I::Outer>,
    Sd: Seeder<Outer = I::Outer, Inner = I::Pos>,
{
    type Pos = I::Outer;
    type Outer = O;
    type Prepared = ();

    fn prepare(&self, _outer: O) {}

    fn apply(&self, _prepared: &(), _best: &BestSoFar<I::Outer>, rng: &mut dyn RngCore, p: ParticleView<'_, I::Outer>) {
        let outer = *p.pos;
        // The inner search's stream is taken from this rule's own stream.
        let key = rng.next_u64();
        let mut seeder_rng = stream(key, u64::MAX);
        let mut particles = self.seeder.seed(outer, &mut seeder_rng);

        let found = run_search(&self.inner, outer, self.iterations, &mut particles, key);
        if found.best_score > f64::NEG_INFINITY {
            let old = <I::Pos as ReadFrom>::read(&*p.pos);
            <I::Pos as WriteInto>::write(found.best_pos, &mut *p.pos);
            <I::Pos as WriteInto>::write(found.best_pos - old, &mut *p.vel);
        }
    }
}
