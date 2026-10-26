use crate::api::{Boundary, Contextful, FitCalc, GBest, Mover, Particle, ParticleMut, PartOf, Unit};
use crate::common::{divide_from, Searcher, SearcherIter, SolitaryMover};
use rand::Rng;
use rand::distr::StandardUniform;

pub struct SearchConfigGBest {
    pub inertia: f64,
    pub cognitive_coeff: f64,
    pub social_coeff: f64,
}

pub struct GBestSearcherOne<'a, T: Unit, TFit: FitCalc, TU1: Mover> {
    fit_calc: &'a TFit,
    gbest: &'a mut GBest<T>,
    tu1: TU1,
}

pub struct GBestSearcherTwo<'a, T: Unit, TFit: FitCalc, TU1: Mover, TU2: Mover> {
    fit_calc: &'a TFit,
    gbest: &'a mut GBest<T>,
    tu1: TU1,
    tu2: TU2,
}

impl<'a, TUnit, TFit, TU1> GBestSearcherOne<'a, TUnit, TFit, TU1>
where
    TUnit: Unit,
    TFit: FitCalc<T = TUnit>,
    TU1: Mover<TUnit = TUnit>,
{
    pub fn new(
        fit_calc: &'a TFit,
        gbest: &'a mut GBest<TUnit>,
        tu1: TU1,
    ) -> GBestSearcherOne<'a, TUnit, TFit, TU1> {
        GBestSearcherOne {
            fit_calc,
            gbest,
            tu1,
        }
    }
}

impl<'a, TUnit, TFit, TU1, TU2> GBestSearcherTwo<'a, TUnit, TFit, TU1, TU2>
where
    TUnit: Unit,
    TFit: FitCalc<T = TUnit>,
    TU1: Mover,
    TU2: Mover,
    TU1::TUnit: PartOf<TUnit>,
    TU2::TUnit: PartOf<TUnit>,
{
    pub fn new(
        fit_calc: &'a TFit,
        gbest: &'a mut GBest<TUnit>,
        tu1: TU1,
        tu2: TU2,
    ) -> GBestSearcherTwo<'a, TUnit, TFit, TU1, TU2> {
        GBestSearcherTwo {
            fit_calc,
            gbest,
            tu1,
            tu2,
        }
    }
}

pub struct GBestSinglePMover<'a, TRng: Rng, TBoundary: Boundary> {
    config: SearchConfigGBest,
    rng: TRng,
    boundary: &'a TBoundary,
}

impl<'a, TRng: Rng, TBoundary: Boundary> GBestSinglePMover<'a, TRng, TBoundary> {
    pub fn new(config: SearchConfigGBest, rng: TRng, boundary: &'a TBoundary) -> Self {
        GBestSinglePMover::<TRng, TBoundary> {
            config,
            boundary,
            rng,
        }
    }
}

pub struct GBestPSOPMover<'a, TUnit: Unit, TSearcher: Searcher<TUnit = TUnit>> {
    create_search: fn() -> SearcherIter<'a, TUnit, TSearcher>,
}

impl<'a, TUnit: Unit, TSearcher: Searcher<TUnit = TUnit>> Contextful
    for GBestPSOPMover<'a, TUnit, TSearcher>
{
    type TContext = ();

    fn set_context(&mut self, _context: Self::TContext) {}
}

impl<'a, TUnit: Unit, TSearcher: Searcher<TUnit = TUnit>> SolitaryMover
    for GBestPSOPMover<'a, TUnit, TSearcher>
{
    type TUnit = TUnit;

    fn update(&mut self, p: ParticleMut<'_, Self::TUnit>) {
        let iter = (self.create_search)();
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

impl<'a, TRng: Rng, TBoundary: Boundary> Contextful for GBestSinglePMover<'a, TRng, TBoundary> {
    type TContext = ();

    fn set_context(&mut self, _context: Self::TContext) {}
}

impl<'a, T: Unit, TRng: Rng, TBoundary: Boundary<T = T>> Mover
    for GBestSinglePMover<'a, TRng, TBoundary>
{
    type TUnit = T;
    type TCommon = GBest<T>;

    fn update(&mut self, common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>) {
        let p_diff = (*p.best_pos - *p.pos) * self.config.cognitive_coeff;
        let g_diff = (common.best_pos - *p.pos) * self.config.social_coeff;
        let a = *p.vel * self.config.inertia;
        let b = p_diff * self.rng.sample::<f64, StandardUniform>(StandardUniform);
        let c = g_diff * self.rng.sample::<f64, StandardUniform>(StandardUniform);
        let vel = a + b + c;
        let new_pos = *p.pos + vel;

        let old_pos = *p.pos;
        *p.pos = new_pos;

        *p.pos = self.boundary.handle(*p.pos);
        *p.vel = old_pos - *p.pos;
    }
}

impl<TUnit, TFit, TU1> Searcher for GBestSearcherOne<'_, TUnit, TFit, TU1>
where
    TUnit: Unit,
    TFit: FitCalc<T = TUnit>,
    TU1: Mover<TUnit = TUnit, TCommon = GBest<TUnit>>,
{
    type TUnit = TUnit;

    fn next(&mut self, particles: &mut [Particle<Self::TUnit>]) -> &GBest<TUnit> {
        for p in particles.iter_mut() {
            p.fit = self.fit_calc.calculate_fit(p.pos);
            update_gbest(self.gbest, p);
            self.tu1.update(self.gbest, p.as_mut());
        }

        return self.gbest;
    }
}

impl<TUnit, TFit, TU1, TU2> Searcher for GBestSearcherTwo<'_, TUnit, TFit, TU1, TU2>
where
    TUnit: Unit,
    TFit: FitCalc<T = TUnit>,
    TU1: Mover<TCommon = GBest<TUnit>>,
    TU2: Mover<TCommon = GBest<TUnit>>,
    TU1::TUnit: PartOf<TUnit>,
    TU2::TUnit: PartOf<TUnit>,
{
    type TUnit = TUnit;

    fn next(&mut self, particles: &mut [Particle<Self::TUnit>]) -> &GBest<TUnit> {
        for p in particles.iter_mut() {
            p.fit = self.fit_calc.calculate_fit(p.pos);
            update_gbest(self.gbest, p);
            self.tu1.update(self.gbest, divide_from(p.as_mut()));
            self.tu2.update(self.gbest, divide_from(p.as_mut()));
        }

        return self.gbest;
    }
}

fn update_gbest<T: Unit>(gbest: &mut GBest<T>, p: &mut Particle<T>) {
    if p.fit > p.best_fit {
        p.best_pos = p.pos;
        p.best_fit = p.fit;

        if p.fit > gbest.best_fit {
            gbest.best_pos = p.pos;
            gbest.best_fit = p.fit;
        }
    }
}
