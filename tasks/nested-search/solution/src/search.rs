use crate::api::{BestSoFar, OuterAware, Particle, Point};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

pub trait Search: OuterAware + Send + Sync {
    type Pos: Point;

    /// Forgets everything from earlier runs.
    fn begin(&mut self);

    /// Runs one iteration over the swarm and returns the best so far.
    /// `rngs[i]` belongs to `particles[i]`.
    fn step(&mut self, particles: &mut [Particle<Self::Pos>], rngs: &mut [StdRng]) -> BestSoFar<Self::Pos>;
}

/// Runs a search for a fixed number of iterations, yielding the best so far
/// after each one. Each particle gets its own random stream, derived from
/// `seed` and the particle's index, so the results don't depend on how the
/// work is split between threads.
pub struct Run<'a, S: Search> {
    iterations: usize,
    done: usize,
    search: &'a mut S,
    particles: &'a mut [Particle<S::Pos>],
    rngs: Vec<StdRng>,
}

impl<'a, S: Search> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut [Particle<S::Pos>], seed: u64) -> Self {
        search.begin();

        let mut master = StdRng::seed_from_u64(seed);
        let rngs = (0..particles.len()).map(|_| StdRng::seed_from_u64(master.random())).collect();

        Run {
            iterations,
            done: 0,
            search,
            particles,
            rngs,
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
        Some(self.search.step(self.particles, &mut self.rngs))
    }
}
