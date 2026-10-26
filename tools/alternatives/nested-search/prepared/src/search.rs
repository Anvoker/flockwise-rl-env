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
    /// Runs one iteration. `rngs[i]` belongs to `particles[i]`: one stream
    /// for each of the swarm's rules.
    fn step(&mut self, particles: &mut [Particle<P>], rngs: &mut [Streams]) -> BestSoFar<P>;
}

/// One particle's random streams, one per rule slot of its swarm, so what one
/// rule draws never changes what another receives.
pub type Streams = [StdRng; 2];

/// The random stream of one rule slot of particle `index`: a hash of the
/// seed, the index and the slot, so it doesn't depend on anything else.
pub(crate) fn particle_rng(seed: u64, index: usize, slot: u64) -> StdRng {
    let mut z = seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ slot.wrapping_mul(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    StdRng::seed_from_u64(z ^ (z >> 31))
}

pub(crate) fn particle_rngs(seed: u64, count: usize) -> Vec<Streams> {
    (0..count).map(|i| [particle_rng(seed, i, 0), particle_rng(seed, i, 1)]).collect()
}

/// Runs a top-level search, yielding the best so far after each iteration.
pub struct Run<'a, S: Search> {
    left: usize,
    ready: S::Ready,
    particles: &'a mut [Particle<S::Pos>],
    rngs: Vec<Streams>,
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
