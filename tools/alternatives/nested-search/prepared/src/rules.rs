use crate::api::{BestSoFar, ParticleView, ReadFrom, SplitFrom, WriteInto};
use crate::components::{Confine, Move, Region, Rule, Seeder};
use crate::search::{Search, Steps, particle_rngs};
use rand::{Rng, RngCore, SeedableRng};
use std::marker::PhantomData;

/// Confines a rule's result to a region.
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

impl<R, G, O> Rule<O> for Confined<R, G>
where
    R: Rule<O>,
    G: Region<O, Pos = R::Pos>,
    O: Copy,
{
    type Pos = R::Pos;
    type Ready = ConfinedReady<R::Ready, G::Ready>;

    fn prepare(&self, outer: O) -> Self::Ready {
        ConfinedReady(self.rule.prepare(outer), self.region.prepare(outer))
    }
}

pub struct ConfinedReady<R, G>(R, G);

impl<P: Copy, R: Move<P>, G: Confine<P>> Move<P> for ConfinedReady<R, G> {
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, mut p: ParticleView<'_, P>) {
        self.0.apply(best, rng, p.reborrow());
        *p.pos = self.1.confine(*p.pos);
    }
}

/// Runs a rule on field `F` of a larger position.
pub struct FieldRule<R, F> {
    rule: R,
    phantom: PhantomData<fn(F)>,
}

impl<R: Clone, F> Clone for FieldRule<R, F> {
    fn clone(&self) -> Self {
        FieldRule {
            rule: self.rule.clone(),
            phantom: PhantomData,
        }
    }
}

/// `rule.for_field::<Field>()` on anything that can be a rule.
pub trait ForField: Sized {
    fn for_field<F>(self) -> FieldRule<Self, F> {
        FieldRule {
            rule: self,
            phantom: PhantomData,
        }
    }
}

impl<T: Clone + Send + Sync> ForField for T {}

impl<R, F, O> Rule<O> for FieldRule<R, F>
where
    R: Rule<O, Pos = F>,
    F: SplitFrom + ReadFrom<Whole = <F as SplitFrom>::Whole>,
{
    type Pos = <F as SplitFrom>::Whole;
    type Ready = FieldReady<R::Ready, F>;

    fn prepare(&self, outer: O) -> Self::Ready {
        FieldReady(self.rule.prepare(outer), PhantomData)
    }
}

pub struct FieldReady<R, F>(R, PhantomData<fn(F)>);

impl<R, F> Move<<F as SplitFrom>::Whole> for FieldReady<R, F>
where
    R: Move<F>,
    F: SplitFrom + ReadFrom<Whole = <F as SplitFrom>::Whole>,
{
    fn apply(&self, best: &BestSoFar<<F as SplitFrom>::Whole>, rng: &mut dyn RngCore, p: ParticleView<'_, <F as SplitFrom>::Whole>) {
        let field_best = BestSoFar {
            best_pos: <F as ReadFrom>::read(&best.best_pos),
            best_score: best.best_score,
        };
        self.0.apply(&field_best, rng, F::split(p));
    }
}

/// Runs a whole inner search for every particle it moves.
#[derive(Clone)]
pub struct InnerSearch<S, D> {
    iterations: usize,
    search: S,
    seeder: D,
}

impl<S, D> InnerSearch<S, D>
where
    D: Seeder,
    S: Search<Pos = D::Inner, Outer = D::Outer>,
    D::Inner: WriteInto<Whole = D::Outer>,
{
    pub fn new(iterations: usize, search: S, seeder: D) -> Self {
        InnerSearch {
            iterations,
            search,
            seeder,
        }
    }
}

impl<S, D, O> Rule<O> for InnerSearch<S, D>
where
    D: Seeder,
    S: Search<Pos = D::Inner, Outer = D::Outer>,
    D::Inner: WriteInto<Whole = D::Outer>,
{
    type Pos = D::Outer;
    type Ready = InnerSearch<S, D>;

    // Ignores its own outer position: it prepares the inner search for each
    // particle it moves instead.
    fn prepare(&self, _outer: O) -> Self::Ready {
        self.clone()
    }
}

impl<S, D> Move<D::Outer> for InnerSearch<S, D>
where
    D: Seeder,
    S: Search<Pos = D::Inner, Outer = D::Outer>,
    D::Inner: WriteInto<Whole = D::Outer>,
{
    fn apply(&self, _best: &BestSoFar<D::Outer>, rng: &mut dyn RngCore, p: ParticleView<'_, D::Outer>) {
        let outer = *p.pos;
        let mut ready = self.search.prepare(outer);
        // The seeder gets its own stream: this rule's stream lasts the whole
        // run, so whatever the seeder drew from it would shift every later
        // inner search's seed.
        let seed = rng.random();
        let mut seeder_rng = rand::rngs::StdRng::seed_from_u64(rng.random());
        let mut particles = self.seeder.seed(outer, &mut seeder_rng);
        let mut rngs = particle_rngs(seed, particles.len());

        let mut best = None;
        for _ in 0..self.iterations {
            best = Some(ready.step(&mut particles, &mut rngs));
        }
        if let Some(best) = best {
            best.best_pos.write(p.pos);
        }
    }
}
