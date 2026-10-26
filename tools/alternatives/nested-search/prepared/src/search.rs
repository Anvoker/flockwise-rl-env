use crate::api::{BestSoFar, Particle, Point};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// A search, prepared for one outer position at a time.
pub trait Search: Clone + Send + Sync {
    type Pos: Point;
    type Outer: Copy + Send + Sync;
    type Ready: Steps<Self::Pos>;

    fn prepare(&self, outer: Self::Outer) -> Self::Ready;
}

/// A search prepared for one outer position, with its own best so far.
pub trait Steps<P>: Send {
    /// Runs one iteration. `rngs[i]` belongs to `particles[i]`.
    fn step(&mut self, particles: &mut [Particle<P>], rngs: &mut [StdRng]) -> BestSoFar<P>;
}

/// The random stream of particle `index`: a hash of the seed and the index,
/// so it doesn't depend on any other particle.
pub(crate) fn particle_rng(seed: u64, index: usize) -> StdRng {
    let mut z = seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    StdRng::seed_from_u64(z ^ (z >> 31))
}

pub(crate) fn particle_rngs(seed: u64, count: usize) -> Vec<StdRng> {
    (0..count).map(|i| particle_rng(seed, i)).collect()
}

/// Runs a top-level search, yielding the best so far after each iteration.
pub struct Run<'a, S: Search> {
    left: usize,
    ready: S::Ready,
    particles: &'a mut [Particle<S::Pos>],
    rngs: Vec<StdRng>,
}

impl<'a, S: Search<Outer = ()>> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut [Particle<S::Pos>], seed: u64) -> Self {
        let rngs = particle_rngs(seed, particles.len());
        Run {
            left: iterations,
            ready: search.prepare(()),
            particles,
            rngs,
        }
    }
}

impl<S: Search> Iterator for Run<'_, S> {
    type Item = BestSoFar<S::Pos>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        Some(self.ready.step(self.particles, &mut self.rngs))
    }
}
