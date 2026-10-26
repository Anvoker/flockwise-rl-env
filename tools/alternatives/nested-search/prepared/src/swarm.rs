use crate::api::{BestSoFar, Particle, ParticleView, Point};
use crate::components::{Move, Objective, Rule, Score};
use crate::search::{Search, Steps, Streams};
use rand::{Rng, RngCore};
use rayon::prelude::*;
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PsoParams {
    pub inertia: f64,
    pub cognitive: f64,
    pub social: f64,
}

/// A swarm with one rule. `O` is the outer position it runs for.
pub struct Swarm1<Ob, R1, O> {
    objective: Ob,
    r1: R1,
    phantom: PhantomData<fn(O)>,
}

/// A swarm with two rules, applied in order.
pub struct Swarm2<Ob, R1, R2, O> {
    objective: Ob,
    r1: R1,
    r2: R2,
    phantom: PhantomData<fn(O)>,
}

impl<Ob: Clone, R1: Clone, O> Clone for Swarm1<Ob, R1, O> {
    fn clone(&self) -> Self {
        Swarm1::build(self.objective.clone(), self.r1.clone())
    }
}

impl<Ob: Clone, R1: Clone, R2: Clone, O> Clone for Swarm2<Ob, R1, R2, O> {
    fn clone(&self) -> Self {
        Swarm2 {
            objective: self.objective.clone(),
            r1: self.r1.clone(),
            r2: self.r2.clone(),
            phantom: PhantomData,
        }
    }
}

impl<Ob, R1, O> Swarm1<Ob, R1, O> {
    fn build(objective: Ob, r1: R1) -> Self {
        Swarm1 {
            objective,
            r1,
            phantom: PhantomData,
        }
    }
}

impl<Ob, R1, O> Swarm1<Ob, R1, O>
where
    Ob: Objective<O>,
    R1: Rule<O, Pos = Ob::Pos>,
{
    pub fn new(objective: Ob, r1: R1) -> Self {
        Swarm1::build(objective, r1)
    }
}

impl<Ob, R1, R2, O> Swarm2<Ob, R1, R2, O>
where
    Ob: Objective<O>,
    R1: Rule<O, Pos = Ob::Pos>,
    R2: Rule<O, Pos = Ob::Pos>,
{
    pub fn new(objective: Ob, r1: R1, r2: R2) -> Self {
        Swarm2 {
            objective,
            r1,
            r2,
            phantom: PhantomData,
        }
    }
}

impl<Ob, R1, O> Search for Swarm1<Ob, R1, O>
where
    Ob: Objective<O>,
    R1: Rule<O, Pos = Ob::Pos>,
    O: Copy + Send + Sync,
{
    type Pos = Ob::Pos;
    type Outer = O;
    type Ready = Ready<Ob::Ready, (R1::Ready,), Ob::Pos>;

    fn prepare(&self, outer: O) -> Self::Ready {
        Ready::new(self.objective.prepare(outer), (self.r1.prepare(outer),))
    }
}

impl<Ob, R1, R2, O> Search for Swarm2<Ob, R1, R2, O>
where
    Ob: Objective<O>,
    R1: Rule<O, Pos = Ob::Pos>,
    R2: Rule<O, Pos = Ob::Pos>,
    O: Copy + Send + Sync,
{
    type Pos = Ob::Pos;
    type Outer = O;
    type Ready = Ready<Ob::Ready, (R1::Ready, R2::Ready), Ob::Pos>;

    fn prepare(&self, outer: O) -> Self::Ready {
        Ready::new(self.objective.prepare(outer), (self.r1.prepare(outer), self.r2.prepare(outer)))
    }
}

/// The prepared rules of a swarm, applied in order.
pub trait Rules<P>: Send + Sync {
    fn apply_all(&self, best: &BestSoFar<P>, rngs: &mut Streams, p: &mut Particle<P>);
}

impl<P: Copy, A: Move<P>> Rules<P> for (A,) {
    fn apply_all(&self, best: &BestSoFar<P>, rngs: &mut Streams, p: &mut Particle<P>) {
        self.0.apply(best, &mut rngs[0], p.view());
    }
}

impl<P: Copy, A: Move<P>, B: Move<P>> Rules<P> for (A, B) {
    fn apply_all(&self, best: &BestSoFar<P>, rngs: &mut Streams, p: &mut Particle<P>) {
        self.0.apply(best, &mut rngs[0], p.view());
        self.1.apply(best, &mut rngs[1], p.view());
    }
}

/// A swarm prepared for one search.
pub struct Ready<S, Rs, P> {
    objective: S,
    rules: Rs,
    best: BestSoFar<P>,
}

impl<S, Rs, P: Default> Ready<S, Rs, P> {
    fn new(objective: S, rules: Rs) -> Self {
        Ready {
            objective,
            rules,
            best: BestSoFar::new(),
        }
    }
}

impl<S, Rs, P> Steps<P> for Ready<S, Rs, P>
where
    S: Score<P>,
    Rs: Rules<P>,
    P: Point,
{
    fn step(&mut self, particles: &mut [Particle<P>], rngs: &mut [Streams]) -> BestSoFar<P> {
        let objective = &self.objective;
        particles.par_iter_mut().for_each(|p| p.score = objective.score(p.pos));

        // The best of this iteration: a parallel reduction that keeps the
        // earlier particle on ties, then merged into the best so far.
        let iteration_best = particles
            .par_iter()
            .enumerate()
            .map(|(i, p)| (i, p.score))
            .reduce(|| (usize::MAX, f64::NEG_INFINITY), |a, b| if b.1 > a.1 || (b.1 == a.1 && b.0 < a.0 && b.1 > f64::NEG_INFINITY) { b } else { a });
        if iteration_best.0 != usize::MAX && iteration_best.1 > self.best.best_score {
            self.best = BestSoFar {
                best_pos: particles[iteration_best.0].pos,
                best_score: iteration_best.1,
            };
        }
        particles.par_iter_mut().for_each(|p| {
            if p.score > p.best_score {
                p.best_score = p.score;
                p.best_pos = p.pos;
            }
        });

        let (best, rules) = (&self.best, &self.rules);
        particles
            .par_iter_mut()
            .zip(rngs.par_iter_mut())
            .for_each(|(p, rngs)| rules.apply_all(best, rngs, p));

        self.best
    }
}

/// The standard global-best PSO update, for any outer position.
pub struct PsoRule<P> {
    params: PsoParams,
    phantom: PhantomData<fn(P)>,
}

impl<P> Clone for PsoRule<P> {
    fn clone(&self) -> Self {
        PsoRule::new(self.params)
    }
}

impl<P> PsoRule<P> {
    pub fn new(params: PsoParams) -> Self {
        PsoRule {
            params,
            phantom: PhantomData,
        }
    }
}

impl<P: Point, O> Rule<O> for PsoRule<P> {
    type Pos = P;
    type Ready = PsoRule<P>;

    fn prepare(&self, _outer: O) -> PsoRule<P> {
        self.clone()
    }
}

impl<P: Point> Move<P> for PsoRule<P> {
    fn apply(&self, best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        let PsoParams { inertia, cognitive, social } = self.params;
        let to_own = *p.best_pos - *p.pos;
        let to_best = best.best_pos - *p.pos;
        let pull_own = to_own.map_scalars(|d| d * cognitive * rng.random::<f64>());
        let pull_best = to_best.map_scalars(|d| d * social * rng.random::<f64>());
        *p.vel = *p.vel * inertia + pull_own + pull_best;
        *p.pos = *p.pos + *p.vel;
    }
}
