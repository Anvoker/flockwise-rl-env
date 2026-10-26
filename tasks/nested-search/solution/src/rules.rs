use crate::api::{BestSoFar, OuterAware, ParticleView, Point, ReadFrom, Region, Rule, Seeder, SplitFrom, WriteInto};
use crate::search::{Run, Search};
use rand::Rng;
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
    type Shared = R::Shared;

    fn apply(&mut self, shared: &Self::Shared, mut p: ParticleView<'_, Self::Pos>) {
        self.rule.apply(shared, p.reborrow());
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
    R: Rule<Shared = BestSoFar<<R as Rule>::Pos>>,
    R::Pos: SplitFrom<Whole = W> + ReadFrom<Whole = W>,
{
    type Pos = W;
    type Shared = BestSoFar<W>;

    fn apply(&mut self, shared: &Self::Shared, p: ParticleView<'_, Self::Pos>) {
        let field_best = BestSoFar {
            best_pos: <R::Pos as ReadFrom>::read(&shared.best_pos),
            best_score: shared.best_score,
        };

        self.rule.apply(&field_best, <R::Pos as SplitFrom>::split(p));
    }
}

/// Runs a whole inner search for every particle it moves, and writes the
/// inner result into that particle's field.
#[derive(Clone, Debug)]
pub struct InnerSearch<S, D, R, Outer> {
    iterations: usize,
    search: S,
    seeder: D,
    rng: R,
    phantom: PhantomData<fn() -> Outer>,
}

impl<S, D, R, Outer> InnerSearch<S, D, R, Outer>
where
    D: Seeder,
    D::Inner: WriteInto<Whole = D::Outer>,
    S: Search<Pos = D::Inner> + OuterAware<Outer = D::Outer>,
    R: Rng,
    Outer: Copy,
{
    pub fn new(iterations: usize, search: S, seeder: D, rng: R) -> Self {
        InnerSearch {
            iterations,
            search,
            seeder,
            rng,
            phantom: PhantomData,
        }
    }
}

impl<S, D, R, Outer: Copy> OuterAware for InnerSearch<S, D, R, Outer> {
    type Outer = Outer;

    // Its own outer position isn't needed: it shows the inner search the
    // particle it is moving instead.
    fn see_outer(&mut self, _outer: Self::Outer) {}
}

impl<S, D, R, Outer> Rule for InnerSearch<S, D, R, Outer>
where
    D: Seeder,
    D::Inner: WriteInto<Whole = D::Outer>,
    S: Search<Pos = D::Inner> + OuterAware<Outer = D::Outer>,
    R: Rng,
    Outer: Copy,
{
    type Pos = D::Outer;
    type Shared = BestSoFar<D::Outer>;

    fn apply(&mut self, _shared: &Self::Shared, p: ParticleView<'_, Self::Pos>) {
        self.search.see_outer(*p.pos);

        let mut particles = self.seeder.seed(*p.pos, &mut self.rng);
        let best = Run::new(self.iterations, &mut self.search, &mut particles).last();

        if let Some(best) = best {
            best.best_pos.write(p.pos);
        }
    }
}
