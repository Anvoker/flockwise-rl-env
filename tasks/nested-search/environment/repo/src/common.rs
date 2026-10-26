use crate::api::{BestSoFar, Particle, ParticleView, Point};
use rand::Rng;

pub trait Objective {
    type Pos: Point;

    fn score(&self, pos: Self::Pos) -> f64;
}

pub trait Region {
    type Pos: Point;

    fn confine(&self, pos: Self::Pos) -> Self::Pos;
}

pub trait Rule {
    type Pos: Point;
    type Shared;

    fn apply<R: Rng + ?Sized>(&self, shared: &Self::Shared, rng: &mut R, p: ParticleView<'_, Self::Pos>);
}

pub trait Seeder {
    type Outer: Point;
    type Inner: Point;

    fn seed<R: Rng + ?Sized>(&self, outer: Self::Outer, rng: &mut R) -> Vec<Particle<Self::Inner>>;
}

pub trait SoloRule {
    type Pos: Point;

    fn apply(&self, p: ParticleView<'_, Self::Pos>);
}

impl<T: SoloRule<Pos = Pos>, Pos: Point> Rule for T {
    type Pos = Pos;
    type Shared = ();

    fn apply<R: Rng + ?Sized>(&self, _shared: &Self::Shared, _rng: &mut R, p: ParticleView<'_, Self::Pos>) {
        SoloRule::apply(self, p);
    }
}

pub trait Search {
    type Pos: Point;

    fn next(&mut self, particles: &mut [Particle<Self::Pos>]) -> &BestSoFar<Self::Pos>;
}

pub struct Run<'a, Pos: Point, S: Search<Pos = Pos>> {
    iterations: usize,
    iteration: usize,
    search: &'a mut S,
    particles: &'a mut [Particle<Pos>],
}

impl<'a, Pos: Point, S: Search<Pos = Pos>> Run<'a, Pos, S> {
    pub fn new(
        iterations: usize,
        search: &'a mut S,
        particles: &'a mut [Particle<Pos>],
    ) -> Self {
        Run {
            iterations,
            iteration: 0,
            search,
            particles,
        }
    }
}

impl<Pos: Point, S: Search<Pos = Pos>> Iterator
    for Run<'_, Pos, S>
{
    type Item = BestSoFar<Pos>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iteration += 1;

        if self.iteration >= self.iterations {
            None
        } else {
            let r = self.search.next(self.particles);
            Some(*r)
        }
    }
}
