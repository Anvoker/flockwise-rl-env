use crate::api::{BestSoFar, Particle, ParticleView, Point};
use crate::components::{Objective, OuterAware, Rule};
use crate::search::{Search, stream};
use rand::{Rng, RngCore};
use rayon::prelude::*;
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PsoParams {
    pub inertia: f64,
    pub cognitive: f64,
    pub social: f64,
}

/// A global-best swarm with one rule.
#[derive(Clone, Debug)]
pub struct Swarm1<O: Objective, R1> {
    objective: O,
    best: BestSoFar<O::Pos>,
    r1: R1,
}

/// A global-best swarm with two rules, applied in order.
#[derive(Clone, Debug)]
pub struct Swarm2<O: Objective, R1, R2> {
    objective: O,
    best: BestSoFar<O::Pos>,
    r1: R1,
    r2: R2,
}

impl<O, R1> Swarm1<O, R1>
where
    O: Objective,
    R1: Rule<Pos = O::Pos, Outer = O::Outer>,
{
    pub fn new(objective: O, r1: R1) -> Self {
        Swarm1 {
            objective,
            best: BestSoFar::new(),
            r1,
        }
    }
}

impl<O, R1, R2> Swarm2<O, R1, R2>
where
    O: Objective,
    R1: Rule<Pos = O::Pos, Outer = O::Outer>,
    R2: Rule<Pos = O::Pos, Outer = O::Outer>,
{
    pub fn new(objective: O, r1: R1, r2: R2) -> Self {
        Swarm2 {
            objective,
            best: BestSoFar::new(),
            r1,
            r2,
        }
    }
}

impl<O, R1> OuterAware for Swarm1<O, R1>
where
    O: Objective,
    R1: Rule<Outer = O::Outer>,
{
    type Outer = O::Outer;

    fn see_outer(&mut self, outer: Self::Outer) {
        self.objective.see_outer(outer);
        self.r1.see_outer(outer);
    }
}

impl<O, R1, R2> OuterAware for Swarm2<O, R1, R2>
where
    O: Objective,
    R1: Rule<Outer = O::Outer>,
    R2: Rule<Outer = O::Outer>,
{
    type Outer = O::Outer;

    fn see_outer(&mut self, outer: Self::Outer) {
        self.objective.see_outer(outer);
        self.r1.see_outer(outer);
        self.r2.see_outer(outer);
    }
}

impl<O, R1> Search for Swarm1<O, R1>
where
    O: Objective,
    R1: Rule<Pos = O::Pos, Outer = O::Outer>,
{
    type Pos = O::Pos;

    fn begin(&mut self) {
        self.best = BestSoFar::new();
    }

    fn step(&mut self, particles: &mut [Particle<Self::Pos>], seed: u64, iteration: u64) -> BestSoFar<Self::Pos> {
        score_all(&self.objective, &mut self.best, particles);

        let (best, r1) = (&self.best, &self.r1);
        particles.par_iter_mut().enumerate().for_each(|(i, p)| {
            r1.apply(best, &mut stream(seed, iteration, i, 0), p.view());
        });

        self.best
    }
}

impl<O, R1, R2> Search for Swarm2<O, R1, R2>
where
    O: Objective,
    R1: Rule<Pos = O::Pos, Outer = O::Outer>,
    R2: Rule<Pos = O::Pos, Outer = O::Outer>,
{
    type Pos = O::Pos;

    fn begin(&mut self) {
        self.best = BestSoFar::new();
    }

    fn step(&mut self, particles: &mut [Particle<Self::Pos>], seed: u64, iteration: u64) -> BestSoFar<Self::Pos> {
        score_all(&self.objective, &mut self.best, particles);

        // Each rule has its own stream, so what one draws never changes what
        // the other receives.
        let (best, r1, r2) = (&self.best, &self.r1, &self.r2);
        particles.par_iter_mut().enumerate().for_each(|(i, p)| {
            r1.apply(best, &mut stream(seed, iteration, i, 0), p.view());
            r2.apply(best, &mut stream(seed, iteration, i, 1), p.view());
        });

        self.best
    }
}

/// Scores every particle in parallel, then updates the bests in particle
/// order, so an earlier particle wins a tie whatever the thread count.
fn score_all<O: Objective>(objective: &O, best: &mut BestSoFar<O::Pos>, particles: &mut [Particle<O::Pos>]) {
    particles.par_iter_mut().for_each(|p| p.score = objective.score(p.pos));

    for p in particles.iter_mut() {
        if p.score > p.best_score {
            p.best_score = p.score;
            p.best_pos = p.pos;
        }

        if p.score > best.best_score {
            best.best_score = p.score;
            best.best_pos = p.pos;
        }
    }
}

/// The standard global-best PSO update. Ignores the outer position.
#[derive(Debug)]
pub struct PsoRule<P, Outer> {
    params: PsoParams,
    phantom: PhantomData<fn() -> (P, Outer)>,
}

impl<P, Outer> Clone for PsoRule<P, Outer> {
    fn clone(&self) -> Self {
        PsoRule {
            params: self.params,
            phantom: PhantomData,
        }
    }
}

impl<P, Outer> PsoRule<P, Outer>
where
    P: Point,
    Outer: Copy + Send + Sync,
{
    pub fn new(params: PsoParams) -> Self {
        PsoRule {
            params,
            phantom: PhantomData,
        }
    }
}

impl<P, Outer: Copy + Send + Sync> OuterAware for PsoRule<P, Outer> {
    type Outer = Outer;

    fn see_outer(&mut self, _outer: Self::Outer) {}
}

impl<P, Outer> Rule for PsoRule<P, Outer>
where
    P: Point,
    Outer: Copy + Send + Sync,
{
    type Pos = P;

    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, Self::Pos>) {
        let (c1, c2) = (self.params.cognitive, self.params.social);
        let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * c1 * rng.random::<f64>());
        let social = (best.best_pos - *p.pos).map_scalars(|d| d * c2 * rng.random::<f64>());

        *p.vel = *p.vel * self.params.inertia + cognitive + social;
        *p.pos = *p.pos + *p.vel;
    }
}
