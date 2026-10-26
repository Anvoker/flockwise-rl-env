use crate::api::{SplitFrom, Region, OuterAware, Objective, BestSoFar, Rule, Particle, ParticleView, Point};
use crate::common::{Search, Run, SoloRule};
use rand::Rng;
use rand::distr::StandardUniform;

pub struct PsoParams {
    pub inertia: f64,
    pub cognitive: f64,
    pub social: f64,
}

pub struct Swarm1<'a, T: Point, O: Objective, R1: Rule> {
    objective: &'a O,
    best: &'a mut BestSoFar<T>,
    r1: R1,
}

pub struct Swarm2<'a, T: Point, O: Objective, R1: Rule, R2: Rule> {
    objective: &'a O,
    best: &'a mut BestSoFar<T>,
    r1: R1,
    r2: R2,
}

impl<'a, Pos, O, R1> Swarm1<'a, Pos, O, R1>
where
    Pos: Point,
    O: Objective<Pos = Pos>,
    R1: Rule<Pos = Pos>,
{
    pub fn new(
        objective: &'a O,
        best: &'a mut BestSoFar<Pos>,
        r1: R1,
    ) -> Swarm1<'a, Pos, O, R1> {
        Swarm1 {
            objective,
            best,
            r1,
        }
    }
}

impl<'a, Pos, O, R1, R2> Swarm2<'a, Pos, O, R1, R2>
where
    Pos: Point,
    O: Objective<Pos = Pos>,
    R1: Rule,
    R2: Rule,
    R1::Pos: SplitFrom<Whole = Pos>,
    R2::Pos: SplitFrom<Whole = Pos>,
{
    pub fn new(
        objective: &'a O,
        best: &'a mut BestSoFar<Pos>,
        r1: R1,
        r2: R2,
    ) -> Swarm2<'a, Pos, O, R1, R2> {
        Swarm2 {
            objective,
            best,
            r1,
            r2,
        }
    }
}

pub struct PsoRule<'a, G: Region> {
    params: PsoParams,
    region: &'a G,
}

impl<'a, G: Region> PsoRule<'a, G> {
    pub fn new(params: PsoParams, region: &'a G) -> Self {
        PsoRule::<G> {
            params,
            region,
        }
    }
}

pub struct SearchRule<'a, Pos: Point, S: Search<Pos = Pos>> {
    make_search: fn() -> Run<'a, Pos, S>,
}

impl<'a, Pos: Point, S: Search<Pos = Pos>> OuterAware
    for SearchRule<'a, Pos, S>
{
    type Outer = ();

    fn see_outer(&mut self, _outer: Self::Outer) {}
}

impl<'a, Pos: Point, S: Search<Pos = Pos>> SoloRule
    for SearchRule<'a, Pos, S>
{
    type Pos = Pos;

    fn apply(&self, p: ParticleView<'_, Self::Pos>) {
        let iter = (self.make_search)();
        match iter.last() {
            Some(x) => {
                let old_pos = *p.pos;
                *p.pos = x.best_pos;
                *p.vel = old_pos - *p.pos;
            }
            None => panic!(),
        }
    }
}

impl<'a, G: Region> OuterAware for PsoRule<'a, G> {
    type Outer = ();

    fn see_outer(&mut self, _outer: Self::Outer) {}
}

impl<'a, T: Point, G: Region<Pos = T>> Rule
    for PsoRule<'a, G>
{
    type Pos = T;
    type Shared = BestSoFar<T>;

    fn apply<R: Rng + ?Sized>(&self, shared: &Self::Shared, rng: &mut R, p: ParticleView<'_, Self::Pos>) {
        let p_diff = (*p.best_pos - *p.pos) * self.params.cognitive;
        let g_diff = (shared.best_pos - *p.pos) * self.params.social;
        let a = *p.vel * self.params.inertia;
        let b = p_diff * rng.sample::<f64, StandardUniform>(StandardUniform);
        let c = g_diff * rng.sample::<f64, StandardUniform>(StandardUniform);
        let vel = a + b + c;
        let new_pos = *p.pos + vel;

        let old_pos = *p.pos;
        *p.pos = new_pos;

        *p.pos = self.region.confine(*p.pos);
        *p.vel = old_pos - *p.pos;
    }
}

impl<Pos, O, R1> Search for Swarm1<'_, Pos, O, R1>
where
    Pos: Point,
    O: Objective<Pos = Pos>,
    R1: Rule<Pos = Pos, Shared = BestSoFar<Pos>>,
{
    type Pos = Pos;

    fn next(&mut self, particles: &mut [Particle<Self::Pos>]) -> &BestSoFar<Pos> {
        let mut rng = rand::rng();
        for p in particles.iter_mut() {
            p.score = self.objective.score(p.pos);
            update_best(self.best, p);
            self.r1.apply(self.best, &mut rng, p.view());
        }

        return self.best;
    }
}

impl<Pos, O, R1, R2> Search for Swarm2<'_, Pos, O, R1, R2>
where
    Pos: Point,
    O: Objective<Pos = Pos>,
    R1: Rule<Shared = BestSoFar<Pos>>,
    R2: Rule<Shared = BestSoFar<Pos>>,
    R1::Pos: SplitFrom<Whole = Pos>,
    R2::Pos: SplitFrom<Whole = Pos>,
{
    type Pos = Pos;

    fn next(&mut self, particles: &mut [Particle<Self::Pos>]) -> &BestSoFar<Pos> {
        let mut rng = rand::rng();
        for p in particles.iter_mut() {
            p.score = self.objective.score(p.pos);
            update_best(self.best, p);
            self.r1.apply(self.best, &mut rng, <R1::Pos as SplitFrom>::split(p.view()));
            self.r2.apply(self.best, &mut rng, <R2::Pos as SplitFrom>::split(p.view()));
        }

        return self.best;
    }
}

fn update_best<T: Point>(best: &mut BestSoFar<T>, p: &mut Particle<T>) {
    if p.score > p.best_score {
        p.best_pos = p.pos;
        p.best_score = p.score;

        if p.score > best.best_score {
            best.best_pos = p.pos;
            best.best_score = p.score;
        }
    }
}
