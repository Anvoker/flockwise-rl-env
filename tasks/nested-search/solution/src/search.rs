use crate::api::{BestSoFar, Particle, Point};
use crate::components::OuterAware;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// A search over positions of type `Pos`, run for the particle of an
/// enclosing search (`Outer`), or for `()` at the top level.
pub trait Search: OuterAware + Clone + Send + Sync {
    type Pos: Point;

    /// Forgets everything from earlier runs.
    fn begin(&mut self);

    /// Runs iteration `iteration` of a run seeded with `seed`, and returns
    /// the best so far.
    fn step(&mut self, particles: &mut [Particle<Self::Pos>], seed: u64, iteration: u64) -> BestSoFar<Self::Pos>;
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The random stream one rule gets for one particle in one iteration. Every
/// (rule, particle, iteration) has its own, so what one component draws never
/// changes what another receives, and the work can be split between threads
/// in any way.
pub(crate) fn stream(seed: u64, iteration: u64, particle: usize, slot: u64) -> StdRng {
    StdRng::seed_from_u64(mix(mix(mix(mix(seed) ^ iteration) ^ particle as u64) ^ slot))
}

/// Runs a top-level search for a fixed number of iterations, yielding the
/// best so far after each one.
pub struct Run<'a, S: Search> {
    iterations: u64,
    done: u64,
    seed: u64,
    search: &'a mut S,
    particles: &'a mut [Particle<S::Pos>],
}

impl<'a, S: Search<Outer = ()>> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut [Particle<S::Pos>], seed: u64) -> Self {
        search.begin();
        search.see_outer(());

        Run {
            iterations: iterations as u64,
            done: 0,
            seed,
            search,
            particles,
        }
    }
}

/// Runs a search to the end and returns its last best so far.
pub(crate) fn run_to_end<S: Search>(
    iterations: usize,
    search: &mut S,
    particles: &mut [Particle<S::Pos>],
    seed: u64,
) -> Option<BestSoFar<S::Pos>> {
    (0..iterations as u64).map(|i| search.step(particles, seed, i)).last()
}

impl<S: Search> Iterator for Run<'_, S> {
    type Item = BestSoFar<S::Pos>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done >= self.iterations {
            return None;
        }

        let best = self.search.step(self.particles, self.seed, self.done);
        self.done += 1;
        Some(best)
    }
}
