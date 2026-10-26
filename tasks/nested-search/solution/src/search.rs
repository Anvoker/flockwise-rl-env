use crate::api::{BestSoFar, OuterAware, Particle, Point};

pub trait Search: OuterAware {
    type Pos: Point;

    /// Forgets everything from earlier runs.
    fn begin(&mut self);

    /// Runs one iteration over the swarm and returns the best so far.
    fn step(&mut self, particles: &mut [Particle<Self::Pos>]) -> BestSoFar<Self::Pos>;
}

/// Runs a search for a fixed number of iterations, yielding the best so far
/// after each one.
pub struct Run<'a, S: Search> {
    iterations: usize,
    done: usize,
    search: &'a mut S,
    particles: &'a mut [Particle<S::Pos>],
}

impl<'a, S: Search> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut [Particle<S::Pos>]) -> Self {
        search.begin();

        Run {
            iterations,
            done: 0,
            search,
            particles,
        }
    }
}

impl<S: Search> Iterator for Run<'_, S> {
    type Item = BestSoFar<S::Pos>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done >= self.iterations {
            return None;
        }

        self.done += 1;
        Some(self.search.step(self.particles))
    }
}
