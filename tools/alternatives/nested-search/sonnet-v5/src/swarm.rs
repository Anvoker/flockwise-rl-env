use crate::api::{BestSoFar, Particle, Point};
use crate::common::{Objective, PreparedRule, Rule};
use crate::rng::{rule_stream, run_seed};
use rayon::prelude::*;

/// A search: something that can be run, given the position of the particle
/// it runs for (`()` at the top level).
pub trait Search: Send + Sync {
    type Pos: Point;
    type Outer: Copy;

    #[doc(hidden)]
    type Prepared<'a>: PreparedSearch<Self::Pos>
    where
        Self: 'a;

    /// Calls every component's closure, once, for one run of this search.
    #[doc(hidden)]
    fn prepare<'a>(&'a self, outer: Self::Outer) -> Self::Prepared<'a>;
}

/// A search ready to iterate.
#[doc(hidden)]
pub trait PreparedSearch<P: Point>: Sync {
    fn step(&self, seed: u64, iteration: u64, best: &mut BestSoFar<P>, particles: &mut [Particle<P>]);
}

fn step_with<P: Point>(
    objective: &(impl Fn(P) -> f64 + Sync),
    rules: &[&dyn PreparedRule<P>],
    seed: u64,
    iteration: u64,
    best: &mut BestSoFar<P>,
    particles: &mut [Particle<P>],
) {
    particles.par_iter_mut().for_each(|p| p.score = objective(p.pos));

    for p in particles.iter_mut() {
        if p.score > p.best_score {
            p.best_pos = p.pos;
            p.best_score = p.score;
        }
        if p.score > best.best_score {
            best.best_pos = p.pos;
            best.best_score = p.score;
        }
    }

    let best: &BestSoFar<P> = best;
    particles.par_iter_mut().enumerate().with_max_len(1).for_each(|(i, p)| {
        for (slot, rule) in rules.iter().enumerate() {
            let mut rng = rule_stream(seed, iteration, slot, i);
            rule.apply(best, &mut rng, p.view());
        }
    });
}

/// A search with one rule.
#[derive(Clone)]
pub struct Swarm1<Ob, R1> {
    objective: Ob,
    r1: R1,
}

impl<Ob, R1> Swarm1<Ob, R1> {
    pub fn new(objective: Ob, r1: R1) -> Self {
        Swarm1 { objective, r1 }
    }
}

pub struct Prepared1<Ob, A1> {
    objective: Ob,
    r1: A1,
}

impl<Ob, R1> Search for Swarm1<Ob, R1>
where
    Ob: Objective,
    R1: Rule<Ob::Pos, Ob::Outer>,
{
    type Pos = Ob::Pos;
    type Outer = Ob::Outer;
    type Prepared<'a>
        = Prepared1<Ob::Prepared<'a>, <R1 as Rule<Ob::Pos, Ob::Outer>>::Prepared<'a>>
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: Self::Outer) -> Self::Prepared<'a> {
        Prepared1 { objective: self.objective.prepare(outer), r1: self.r1.prepare(outer) }
    }
}

impl<P, Ob, A1> PreparedSearch<P> for Prepared1<Ob, A1>
where
    P: Point,
    Ob: Fn(P) -> f64 + Sync,
    A1: PreparedRule<P>,
{
    fn step(&self, seed: u64, iteration: u64, best: &mut BestSoFar<P>, particles: &mut [Particle<P>]) {
        step_with(&self.objective, &[&self.r1], seed, iteration, best, particles);
    }
}

/// A search with two rules, applied in order.
#[derive(Clone)]
pub struct Swarm2<Ob, R1, R2> {
    objective: Ob,
    r1: R1,
    r2: R2,
}

impl<Ob, R1, R2> Swarm2<Ob, R1, R2> {
    pub fn new(objective: Ob, r1: R1, r2: R2) -> Self {
        Swarm2 { objective, r1, r2 }
    }
}

pub struct Prepared2<Ob, A1, A2> {
    objective: Ob,
    r1: A1,
    r2: A2,
}

impl<Ob, R1, R2> Search for Swarm2<Ob, R1, R2>
where
    Ob: Objective,
    R1: Rule<Ob::Pos, Ob::Outer>,
    R2: Rule<Ob::Pos, Ob::Outer>,
{
    type Pos = Ob::Pos;
    type Outer = Ob::Outer;
    type Prepared<'a>
        = Prepared2<
        Ob::Prepared<'a>,
        <R1 as Rule<Ob::Pos, Ob::Outer>>::Prepared<'a>,
        <R2 as Rule<Ob::Pos, Ob::Outer>>::Prepared<'a>,
    >
    where
        Self: 'a;

    fn prepare<'a>(&'a self, outer: Self::Outer) -> Self::Prepared<'a> {
        Prepared2 {
            objective: self.objective.prepare(outer),
            r1: self.r1.prepare(outer),
            r2: self.r2.prepare(outer),
        }
    }
}

impl<P, Ob, A1, A2> PreparedSearch<P> for Prepared2<Ob, A1, A2>
where
    P: Point,
    Ob: Fn(P) -> f64 + Sync,
    A1: PreparedRule<P>,
    A2: PreparedRule<P>,
{
    fn step(&self, seed: u64, iteration: u64, best: &mut BestSoFar<P>, particles: &mut [Particle<P>]) {
        step_with(&self.objective, &[&self.r1, &self.r2], seed, iteration, best, particles);
    }
}

/// Runs a search that isn't nested, as an iterator over the best so far
/// after each iteration.
pub struct Run<'a, S: Search<Outer = ()> + 'a> {
    iterations: usize,
    iteration: usize,
    seed: u64,
    prepared: S::Prepared<'a>,
    best: BestSoFar<S::Pos>,
    particles: &'a mut Vec<Particle<S::Pos>>,
}

impl<'a, S: Search<Outer = ()>> Run<'a, S> {
    pub fn new(iterations: usize, search: &'a mut S, particles: &'a mut Vec<Particle<S::Pos>>, seed: u64) -> Self {
        let search: &'a S = search;
        Run {
            iterations,
            iteration: 0,
            seed: run_seed(seed),
            prepared: search.prepare(()),
            best: BestSoFar::new(),
            particles,
        }
    }
}

impl<S: Search<Outer = ()>> Iterator for Run<'_, S> {
    type Item = BestSoFar<S::Pos>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iteration >= self.iterations {
            return None;
        }
        self.prepared.step(self.seed, self.iteration as u64, &mut self.best, self.particles);
        self.iteration += 1;
        Some(self.best)
    }
}
