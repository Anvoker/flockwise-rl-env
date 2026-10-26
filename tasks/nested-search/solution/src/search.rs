use crate::api::{BestSoFar, Particle, Point};
use crate::components::OuterAware;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// A search over positions of type `Pos`, run for the particle of an
/// enclosing search (`Outer`), or for `()` at the top level.
pub trait Search: OuterAware + Clone + Send + Sync {
    type Pos: Point;

    /// Forgets everything from earlier runs.
    fn begin(&mut self);

    /// Runs one iteration over the swarm and returns the best so far.
    /// `rngs[i]` belongs to `particles[i]`.
    fn step(&mut self, particles: &mut [Particle<Self::Pos>], rngs: &mut [StdRng]) -> BestSoFar<Self::Pos>;
}

/// Runs a top-level search for a fixed number of iterations, yielding the
/// best so far after each one. Each particle gets its own random stream,
/// derived from `seed` and the particle's index, so the results don't depend
/// on how the work is split between threads.
pub struct Run<'a, S: Search> {
    iterations: usize,
    done: usize,
    search: &'a mut S,
    particles: &'a mut [Particle<S::Pos>],
    rngs: Vec<StdRng>,
}

impl<'a, S: Search<Outer = ()>> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut [Particle<S::Pos>], seed: u64) -> Self {
        search.begin();
        search.see_outer(());
        let rngs = particle_rngs(seed, particles.len());

        Run {
            iterations,
            done: 0,
            search,
            particles,
            rngs,
        }
    }
}

/// One random stream per particle, from a master stream seeded with `seed`.
pub(crate) fn particle_rngs(seed: u64, count: usize) -> Vec<StdRng> {
    let mut master = StdRng::seed_from_u64(seed);
    (0..count).map(|_| StdRng::seed_from_u64(master.random())).collect()
}

/// Runs a search to the end and returns its last best so far.
pub(crate) fn run_to_end<S: Search>(
    iterations: usize,
    search: &mut S,
    particles: &mut [Particle<S::Pos>],
    seed: u64,
) -> Option<BestSoFar<S::Pos>> {
    let mut rngs = particle_rngs(seed, particles.len());
    (0..iterations).map(|_| search.step(particles, &mut rngs)).last()
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
