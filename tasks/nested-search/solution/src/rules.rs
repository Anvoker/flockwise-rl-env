use crate::api::{BestSoFar, ParticleView, Point, ReadFrom, SplitFrom, WriteInto};
use crate::components::{OuterAware, Region, Rule, Seeder};
use crate::search::{Search, run_to_end};
use rand::{Rng, RngCore};
use std::marker::PhantomData;

/// Confines a rule's result to a region.
#[derive(Clone, Debug)]
pub struct Confined<R, G> {
    rule: R,
    region: G,
}

impl<R, G> Confined<R, G>
where
    G: Region,
    R: Rule<Pos = G::Pos, Outer = G::Outer>,
{
    pub fn new(rule: R, region: G) -> Self {
        Confined { rule, region }
    }
}

impl<R, G> OuterAware for Confined<R, G>
where
    G: Region,
    R: Rule<Outer = G::Outer>,
{
    type Outer = G::Outer;

    fn see_outer(&mut self, outer: Self::Outer) {
        self.rule.see_outer(outer);
        self.region.see_outer(outer);
    }
}

impl<R, G> Rule for Confined<R, G>
where
    G: Region,
    R: Rule<Pos = G::Pos, Outer = G::Outer>,
{
    type Pos = G::Pos;

    fn apply(&self, best: &BestSoFar<Self::Pos>, rng: &mut dyn RngCore, mut p: ParticleView<'_, Self::Pos>) {
        self.rule.apply(best, rng, p.reborrow());
        *p.pos = self.region.confine(*p.pos);
    }
}

/// Runs a rule on one field of a larger position.
#[derive(Clone, Debug)]
pub struct FieldRule<R> {
    rule: R,
}

/// Lets any rule be restricted to one field: `rule.for_field::<Field>()`.
pub trait ForField: Rule + Sized {
    fn for_field<F>(self) -> FieldRule<Self>
    where
        Self: Rule<Pos = F>;
}

impl<R: Rule> ForField for R {
    fn for_field<F>(self) -> FieldRule<Self>
    where
        Self: Rule<Pos = F>,
    {
        FieldRule { rule: self }
    }
}

impl<R: Rule> OuterAware for FieldRule<R> {
    type Outer = R::Outer;

    fn see_outer(&mut self, outer: Self::Outer) {
        self.rule.see_outer(outer);
    }
}

impl<R, W> Rule for FieldRule<R>
where
    W: Point,
    R: Rule,
    R::Pos: SplitFrom<Whole = W> + ReadFrom<Whole = W>,
{
    type Pos = W;

    fn apply(&self, best: &BestSoFar<W>, rng: &mut dyn RngCore, p: ParticleView<'_, W>) {
        let field_best = BestSoFar {
            best_pos: <R::Pos as ReadFrom>::read(&best.best_pos),
            best_score: best.best_score,
        };

        self.rule.apply(&field_best, rng, <R::Pos as SplitFrom>::split(p));
    }
}

/// Runs a whole inner search for every particle it moves, and writes the
/// inner result into that particle's field. Particles can be moved in
/// parallel, so each one gets a fresh copy of the inner search, shown that
/// particle's position.
#[derive(Clone, Debug)]
pub struct InnerSearch<S, D, Outer> {
    iterations: usize,
    search: S,
    seeder: D,
    phantom: PhantomData<fn() -> Outer>,
}

impl<S, D, Outer> InnerSearch<S, D, Outer>
where
    D: Seeder,
    D::Inner: WriteInto<Whole = D::Outer>,
    S: Search<Pos = D::Inner, Outer = D::Outer>,
    Outer: Copy + Send + Sync,
{
    pub fn new(iterations: usize, search: S, seeder: D) -> Self {
        InnerSearch {
            iterations,
            search,
            seeder,
            phantom: PhantomData,
        }
    }
}

impl<S, D, Outer: Copy + Send + Sync> OuterAware for InnerSearch<S, D, Outer> {
    type Outer = Outer;

    // Its own outer position isn't needed: it shows the inner search the
    // particle it is moving instead.
    fn see_outer(&mut self, _outer: Self::Outer) {}
}

impl<S, D, Outer> Rule for InnerSearch<S, D, Outer>
where
    D: Seeder,
    D::Inner: WriteInto<Whole = D::Outer>,
    S: Search<Pos = D::Inner, Outer = D::Outer>,
    Outer: Copy + Send + Sync,
{
    type Pos = D::Outer;

    fn apply(&self, _best: &BestSoFar<Self::Pos>, rng: &mut dyn RngCore, p: ParticleView<'_, Self::Pos>) {
        let mut search = self.search.clone();
        search.begin();
        search.see_outer(*p.pos);

        let seed = rng.random::<u64>();
        let mut particles = self.seeder.seed(*p.pos, rng);

        if let Some(best) = run_to_end(self.iterations, &mut search, &mut particles, seed) {
            best.best_pos.write(p.pos);
        }
    }
}
