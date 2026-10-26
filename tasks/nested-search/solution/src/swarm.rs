use crate::api::{BestSoFar, Objective, OuterAware, Particle, ParticleView, Point, Rule};
use crate::search::Search;
use rand::Rng;
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
    R1: Rule<Pos = O::Pos, Shared = BestSoFar<O::Pos>, Outer = O::Outer>,
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
    R1: Rule<Pos = O::Pos, Shared = BestSoFar<O::Pos>, Outer = O::Outer>,
    R2: Rule<Pos = O::Pos, Shared = BestSoFar<O::Pos>, Outer = O::Outer>,
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
    R1: Rule<Pos = O::Pos, Shared = BestSoFar<O::Pos>, Outer = O::Outer>,
{
    type Pos = O::Pos;

    fn begin(&mut self) {
        self.best = BestSoFar::new();
    }

    fn step(&mut self, particles: &mut [Particle<Self::Pos>]) -> BestSoFar<Self::Pos> {
        for p in particles.iter_mut() {
            score(&self.objective, &mut self.best, p);
            self.r1.apply(&self.best, p.view());
        }

        self.best
    }
}

impl<O, R1, R2> Search for Swarm2<O, R1, R2>
where
    O: Objective,
    R1: Rule<Pos = O::Pos, Shared = BestSoFar<O::Pos>, Outer = O::Outer>,
    R2: Rule<Pos = O::Pos, Shared = BestSoFar<O::Pos>, Outer = O::Outer>,
{
    type Pos = O::Pos;

    fn begin(&mut self) {
        self.best = BestSoFar::new();
    }

    fn step(&mut self, particles: &mut [Particle<Self::Pos>]) -> BestSoFar<Self::Pos> {
        for p in particles.iter_mut() {
            score(&self.objective, &mut self.best, p);
            self.r1.apply(&self.best, p.view());
            self.r2.apply(&self.best, p.view());
        }

        self.best
    }
}

fn score<O: Objective>(objective: &O, best: &mut BestSoFar<O::Pos>, p: &mut Particle<O::Pos>) {
    p.score = objective.score(p.pos);

    if p.score > p.best_score {
        p.best_score = p.score;
        p.best_pos = p.pos;
    }

    if p.score > best.best_score {
        best.best_score = p.score;
        best.best_pos = p.pos;
    }
}

/// The standard global-best PSO update. Ignores the outer position.
#[derive(Clone, Debug)]
pub struct PsoRule<P, R, Outer> {
    params: PsoParams,
    rng: R,
    phantom: PhantomData<fn() -> (P, Outer)>,
}

impl<P, R, Outer> PsoRule<P, R, Outer>
where
    P: Point,
    R: Rng,
    Outer: Copy,
{
    pub fn new(params: PsoParams, rng: R) -> Self {
        PsoRule {
            params,
            rng,
            phantom: PhantomData,
        }
    }
}

impl<P, R, Outer: Copy> OuterAware for PsoRule<P, R, Outer> {
    type Outer = Outer;

    fn see_outer(&mut self, _outer: Self::Outer) {}
}

impl<P, R, Outer> Rule for PsoRule<P, R, Outer>
where
    P: Point,
    R: Rng,
    Outer: Copy,
{
    type Pos = P;
    type Shared = BestSoFar<P>;

    fn apply(&mut self, shared: &Self::Shared, p: ParticleView<'_, Self::Pos>) {
        let rng = &mut self.rng;
        let (c1, c2) = (self.params.cognitive, self.params.social);
        let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * c1 * rng.random::<f64>());
        let social = (shared.best_pos - *p.pos).map_scalars(|d| d * c2 * rng.random::<f64>());

        *p.vel = *p.vel * self.params.inertia + cognitive + social;
        *p.pos = *p.pos + *p.vel;
    }
}
