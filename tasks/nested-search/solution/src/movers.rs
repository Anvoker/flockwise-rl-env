use crate::api::{Boundary, Contextful, GBest, Mover, NestedInit, ParticleMut, PartOf, Unit};
use crate::searcher::{Searcher, SearcherIter};
use rand::Rng;
use std::marker::PhantomData;

/// Applies a boundary after another mover.
#[derive(Clone, Debug)]
pub struct BoundedMover<TMover, TBoundary> {
    mover: TMover,
    boundary: TBoundary,
}

impl<TMover, TBoundary> BoundedMover<TMover, TBoundary>
where
    TBoundary: Boundary,
    TMover: Mover<TUnit = TBoundary::T, TContext = TBoundary::TContext>,
{
    pub fn new(mover: TMover, boundary: TBoundary) -> Self {
        BoundedMover { mover, boundary }
    }
}

impl<TMover, TBoundary> Contextful for BoundedMover<TMover, TBoundary>
where
    TBoundary: Boundary,
    TMover: Mover<TContext = TBoundary::TContext>,
{
    type TContext = TBoundary::TContext;

    fn set_context(&mut self, context: Self::TContext) {
        self.mover.set_context(context);
        self.boundary.set_context(context);
    }
}

impl<TMover, TBoundary> Mover for BoundedMover<TMover, TBoundary>
where
    TBoundary: Boundary,
    TMover: Mover<TUnit = TBoundary::T, TContext = TBoundary::TContext>,
{
    type TUnit = TBoundary::T;
    type TCommon = TMover::TCommon;

    fn update(&mut self, common: &Self::TCommon, mut p: ParticleMut<'_, Self::TUnit>) {
        self.mover.update(common, p.reborrow());
        *p.pos = self.boundary.handle(*p.pos);
    }
}

/// Runs a mover on one part of a larger unit.
#[derive(Clone, Debug)]
pub struct PartMover<TMover, TWhole> {
    mover: TMover,
    phantom: PhantomData<fn() -> TWhole>,
}

/// Lets any mover be restricted to a part: `mover.on_part::<Part>()`.
pub trait OnPart<TWhole>: Sized {
    fn on_part<TPart>(self) -> PartMover<Self, TWhole>
    where
        Self: Mover<TUnit = TPart>;
}

impl<TMover: Mover, TWhole> OnPart<TWhole> for TMover {
    fn on_part<TPart>(self) -> PartMover<Self, TWhole>
    where
        Self: Mover<TUnit = TPart>,
    {
        PartMover {
            mover: self,
            phantom: PhantomData,
        }
    }
}

impl<TMover: Mover, TWhole> Contextful for PartMover<TMover, TWhole> {
    type TContext = TMover::TContext;

    fn set_context(&mut self, context: Self::TContext) {
        self.mover.set_context(context);
    }
}

impl<TMover, TWhole> Mover for PartMover<TMover, TWhole>
where
    TWhole: Unit,
    TMover: Mover<TCommon = GBest<<TMover as Mover>::TUnit>>,
    TMover::TUnit: PartOf<TWhole>,
{
    type TUnit = TWhole;
    type TCommon = GBest<TWhole>;

    fn update(&mut self, common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>) {
        let part_common = GBest {
            best_pos: TMover::TUnit::get(&common.best_pos),
            best_fit: common.best_fit,
        };

        let part = ParticleMut {
            pos: TMover::TUnit::get_mut(p.pos),
            vel: TMover::TUnit::get_mut(p.vel),
            fit: p.fit,
            best_pos: TMover::TUnit::get_mut(p.best_pos),
            best_fit: p.best_fit,
        };

        self.mover.update(&part_common, part);
    }
}

/// Runs a whole inner search for every particle it moves, and writes the
/// inner result into that particle's part.
#[derive(Clone, Debug)]
pub struct NestedMover<TSearcher, TInit, TRng, TContext> {
    max_iteration: usize,
    searcher: TSearcher,
    init: TInit,
    rng: TRng,
    phantom: PhantomData<fn() -> TContext>,
}

impl<TSearcher, TInit, TRng, TContext> NestedMover<TSearcher, TInit, TRng, TContext>
where
    TInit: NestedInit,
    TInit::TInner: PartOf<TInit::TOuter>,
    TSearcher: Searcher<TUnit = TInit::TInner> + Contextful<TContext = TInit::TOuter>,
    TRng: Rng,
    TContext: Copy,
{
    pub fn new(max_iteration: usize, searcher: TSearcher, init: TInit, rng: TRng) -> Self {
        NestedMover {
            max_iteration,
            searcher,
            init,
            rng,
            phantom: PhantomData,
        }
    }
}

impl<TSearcher, TInit, TRng, TContext: Copy> Contextful
    for NestedMover<TSearcher, TInit, TRng, TContext>
{
    type TContext = TContext;

    // The nested mover's own context is not needed: it gives its inner search
    // the particle it is moving instead.
    fn set_context(&mut self, _context: Self::TContext) {}
}

impl<TSearcher, TInit, TRng, TContext> Mover for NestedMover<TSearcher, TInit, TRng, TContext>
where
    TInit: NestedInit,
    TInit::TInner: PartOf<TInit::TOuter>,
    TSearcher: Searcher<TUnit = TInit::TInner> + Contextful<TContext = TInit::TOuter>,
    TRng: Rng,
    TContext: Copy,
{
    type TUnit = TInit::TOuter;
    type TCommon = GBest<TInit::TOuter>;

    fn update(&mut self, _common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>) {
        self.searcher.set_context(*p.pos);

        let mut group = self.init.init(*p.pos, &mut self.rng);
        let best = SearcherIter::new(self.max_iteration, &mut self.searcher, &mut group).last();

        if let Some(best) = best {
            *TInit::TInner::get_mut(p.pos) = best.best_pos;
        }
    }
}
