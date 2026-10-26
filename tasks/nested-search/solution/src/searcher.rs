use crate::api::{Contextful, GBest, Particle, Unit};

pub trait Searcher: Contextful {
    type TUnit: Unit;

    /// Forgets everything from previous searches.
    fn begin(&mut self);

    /// Runs one iteration over the swarm and returns the global best so far.
    fn next(&mut self, particles: &mut [Particle<Self::TUnit>]) -> GBest<Self::TUnit>;
}

/// Runs a search for a fixed number of iterations, yielding the global best
/// after each one.
pub struct SearcherIter<'a, S: Searcher> {
    max_iteration: usize,
    iteration: usize,
    searcher: &'a mut S,
    group: &'a mut [Particle<S::TUnit>],
}

impl<'a, S: Searcher> SearcherIter<'a, S> {
    pub fn new(
        max_iteration: usize,
        searcher: &'a mut S,
        group: &'a mut [Particle<S::TUnit>],
    ) -> Self {
        searcher.begin();

        SearcherIter {
            max_iteration,
            iteration: 0,
            searcher,
            group,
        }
    }
}

impl<S: Searcher> Iterator for SearcherIter<'_, S> {
    type Item = GBest<S::TUnit>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iteration >= self.max_iteration {
            return None;
        }

        self.iteration += 1;
        Some(self.searcher.next(self.group))
    }
}
