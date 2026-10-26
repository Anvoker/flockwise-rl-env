use crate::api::{Contextful, GBest, Mover, Particle, ParticleMut, PartOf, Unit};

pub trait SolitaryMover {
    type TUnit: Unit;

    fn update(&mut self, p: ParticleMut<'_, Self::TUnit>);
}

impl<T: SolitaryMover<TUnit = TUnit> + Contextful, TUnit: Unit> Mover for T {
    type TUnit = TUnit;
    type TCommon = ();

    fn update(&mut self, _common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>) {
        SolitaryMover::update(self, p);
    }
}

/// A view of one part of a particle.
pub fn divide_from<TPart: PartOf<TWhole>, TWhole: Unit>(
    source: ParticleMut<'_, TWhole>,
) -> ParticleMut<'_, TPart> {
    ParticleMut {
        pos: TPart::get_mut(source.pos),
        vel: TPart::get_mut(source.vel),
        fit: source.fit,
        best_pos: TPart::get_mut(source.best_pos),
        best_fit: source.best_fit,
    }
}

pub trait Searcher {
    type TUnit: Unit;

    fn next(&mut self, particles: &mut [Particle<Self::TUnit>]) -> &GBest<Self::TUnit>;
}

pub struct SearcherIter<'a, TUnit: Unit, TSearcher: Searcher<TUnit = TUnit>> {
    max_iteration: usize,
    iteration: usize,
    searcher: &'a mut TSearcher,
    group: &'a mut [Particle<TUnit>],
}

impl<'a, TUnit: Unit, TSearcher: Searcher<TUnit = TUnit>> SearcherIter<'a, TUnit, TSearcher> {
    pub fn new(
        max_iteration: usize,
        searcher: &'a mut TSearcher,
        group: &'a mut [Particle<TUnit>],
    ) -> Self {
        SearcherIter {
            max_iteration,
            iteration: 0,
            searcher,
            group,
        }
    }
}

impl<TUnit: Unit, TSearcher: Searcher<TUnit = TUnit>> Iterator
    for SearcherIter<'_, TUnit, TSearcher>
{
    type Item = GBest<TUnit>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iteration += 1;

        if self.iteration >= self.max_iteration {
            None
        } else {
            let r = self.searcher.next(self.group);
            Some(*r)
        }
    }
}
