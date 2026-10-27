use crate::api::{BestSoFar, Particle, ParticleView, Point};
use crate::common::{Objective, Rule, Search, child, score_particles, stream};
use rand::{Rng, RngCore};
use rayon::prelude::*;
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PsoParams {
    pub inertia: f64,
    pub cognitive: f64,
    pub social: f64,
}

/// A search with one rule.
#[derive(Clone)]
pub struct Swarm1<Obj, R1> {
    objective: Obj,
    rule: R1,
}

/// A search with two rules, applied in that order to every particle.
#[derive(Clone)]
pub struct Swarm2<Obj, R1, R2> {
    objective: Obj,
    rule1: R1,
    rule2: R2,
}

impl<Obj, R1> Swarm1<Obj, R1> {
    pub fn new(objective: Obj, rule: R1) -> Self {
        Swarm1 { objective, rule }
    }
}

impl<Obj, R1, R2> Swarm2<Obj, R1, R2> {
    pub fn new(objective: Obj, rule1: R1, rule2: R2) -> Self {
        Swarm2 { objective, rule1, rule2 }
    }
}

impl<Obj, R1> Search for Swarm1<Obj, R1>
where
    Obj: Objective,
    R1: Rule<Pos = Obj::Pos, Outer = Obj::Outer>,
{
    type Pos = Obj::Pos;
    type Outer = Obj::Outer;
    type Prepared = (Obj::Prepared, R1::Prepared);

    fn prepare(&self, outer: Obj::Outer) -> Self::Prepared {
        (self.objective.prepare(outer), self.rule.prepare(outer))
    }

    fn step(&self, prepared: &Self::Prepared, key: u64, best: &mut BestSoFar<Obj::Pos>, particles: &mut [Particle<Obj::Pos>]) {
        score_particles(&prepared.0, best, particles);

        let snapshot = *best;
        let rule_key = child(key, 1);
        particles.par_iter_mut().enumerate().for_each(|(i, p)| {
            let mut rng = stream(rule_key, i as u64);
            self.rule.apply(&prepared.1, &snapshot, &mut rng, p.view());
        });
    }
}

impl<Obj, R1, R2> Search for Swarm2<Obj, R1, R2>
where
    Obj: Objective,
    R1: Rule<Pos = Obj::Pos, Outer = Obj::Outer>,
    R2: Rule<Pos = Obj::Pos, Outer = Obj::Outer>,
{
    type Pos = Obj::Pos;
    type Outer = Obj::Outer;
    type Prepared = (Obj::Prepared, R1::Prepared, R2::Prepared);

    fn prepare(&self, outer: Obj::Outer) -> Self::Prepared {
        (self.objective.prepare(outer), self.rule1.prepare(outer), self.rule2.prepare(outer))
    }

    fn step(&self, prepared: &Self::Prepared, key: u64, best: &mut BestSoFar<Obj::Pos>, particles: &mut [Particle<Obj::Pos>]) {
        score_particles(&prepared.0, best, particles);

        let snapshot = *best;
        let (key1, key2) = (child(key, 1), child(key, 2));
        particles.par_iter_mut().enumerate().for_each(|(i, p)| {
            let mut rng = stream(key1, i as u64);
            self.rule1.apply(&prepared.1, &snapshot, &mut rng, p.view());
            let mut rng = stream(key2, i as u64);
            self.rule2.apply(&prepared.2, &snapshot, &mut rng, p.view());
        });
    }
}

/// Standard global-best PSO, without any confinement: `Confined` does that.
#[derive(Copy, Clone, Debug)]
pub struct PsoRule<O, P> {
    params: PsoParams,
    marker: PhantomData<fn(O) -> P>,
}

impl<O, P> PsoRule<O, P> {
    pub fn new(params: PsoParams) -> Self {
        PsoRule { params, marker: PhantomData }
    }
}

impl<O, P> Rule for PsoRule<O, P>
where
    O: Copy,
    P: Point,
{
    type Pos = P;
    type Outer = O;
    type Prepared = ();

    fn prepare(&self, _outer: O) {}

    fn apply(&self, _prepared: &(), best: &BestSoFar<P>, rng: &mut dyn RngCore, p: ParticleView<'_, P>) {
        let PsoParams { inertia, cognitive, social } = self.params;

        // One random number per coordinate for each of the two pulls.
        let to_personal = (*p.best_pos - *p.pos) * cognitive;
        let to_global = (best.best_pos - *p.pos) * social;
        let personal_pull = to_personal.map_scalars(|d| d * rng.random::<f64>());
        let global_pull = to_global.map_scalars(|d| d * rng.random::<f64>());

        let vel = *p.vel * inertia + personal_pull + global_pull;
        *p.vel = vel;
        *p.pos = *p.pos + vel;
    }
}
