use crate::api::{Contextful, FitCalc, GBest, Mover, Particle, ParticleMut, Unit};
use crate::searcher::Searcher;
use rand::Rng;
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SearchConfigGBest {
    pub inertia: f64,
    pub cognitive_coeff: f64,
    pub social_coeff: f64,
}

/// A global-best searcher with one mover.
#[derive(Clone, Debug)]
pub struct GBestSearcherOne<TFit: FitCalc, TU1> {
    fit_calc: TFit,
    gbest: GBest<TFit::T>,
    tu1: TU1,
}

/// A global-best searcher with two movers, applied in order.
#[derive(Clone, Debug)]
pub struct GBestSearcherTwo<TFit: FitCalc, TU1, TU2> {
    fit_calc: TFit,
    gbest: GBest<TFit::T>,
    tu1: TU1,
    tu2: TU2,
}

impl<TFit, TU1> GBestSearcherOne<TFit, TU1>
where
    TFit: FitCalc,
    TU1: Mover<TUnit = TFit::T, TCommon = GBest<TFit::T>, TContext = TFit::TContext>,
{
    pub fn new(fit_calc: TFit, tu1: TU1) -> Self {
        GBestSearcherOne {
            fit_calc,
            gbest: GBest::new(),
            tu1,
        }
    }
}

impl<TFit, TU1, TU2> GBestSearcherTwo<TFit, TU1, TU2>
where
    TFit: FitCalc,
    TU1: Mover<TUnit = TFit::T, TCommon = GBest<TFit::T>, TContext = TFit::TContext>,
    TU2: Mover<TUnit = TFit::T, TCommon = GBest<TFit::T>, TContext = TFit::TContext>,
{
    pub fn new(fit_calc: TFit, tu1: TU1, tu2: TU2) -> Self {
        GBestSearcherTwo {
            fit_calc,
            gbest: GBest::new(),
            tu1,
            tu2,
        }
    }
}

impl<TFit, TU1> Contextful for GBestSearcherOne<TFit, TU1>
where
    TFit: FitCalc,
    TU1: Mover<TContext = TFit::TContext>,
{
    type TContext = TFit::TContext;

    fn set_context(&mut self, context: Self::TContext) {
        self.fit_calc.set_context(context);
        self.tu1.set_context(context);
    }
}

impl<TFit, TU1, TU2> Contextful for GBestSearcherTwo<TFit, TU1, TU2>
where
    TFit: FitCalc,
    TU1: Mover<TContext = TFit::TContext>,
    TU2: Mover<TContext = TFit::TContext>,
{
    type TContext = TFit::TContext;

    fn set_context(&mut self, context: Self::TContext) {
        self.fit_calc.set_context(context);
        self.tu1.set_context(context);
        self.tu2.set_context(context);
    }
}

impl<TFit, TU1> Searcher for GBestSearcherOne<TFit, TU1>
where
    TFit: FitCalc,
    TU1: Mover<TUnit = TFit::T, TCommon = GBest<TFit::T>, TContext = TFit::TContext>,
{
    type TUnit = TFit::T;

    fn begin(&mut self) {
        self.gbest = GBest::new();
    }

    fn next(&mut self, particles: &mut [Particle<Self::TUnit>]) -> GBest<Self::TUnit> {
        for p in particles.iter_mut() {
            evaluate(&self.fit_calc, &mut self.gbest, p);
            self.tu1.update(&self.gbest, p.as_mut());
        }

        self.gbest
    }
}

impl<TFit, TU1, TU2> Searcher for GBestSearcherTwo<TFit, TU1, TU2>
where
    TFit: FitCalc,
    TU1: Mover<TUnit = TFit::T, TCommon = GBest<TFit::T>, TContext = TFit::TContext>,
    TU2: Mover<TUnit = TFit::T, TCommon = GBest<TFit::T>, TContext = TFit::TContext>,
{
    type TUnit = TFit::T;

    fn begin(&mut self) {
        self.gbest = GBest::new();
    }

    fn next(&mut self, particles: &mut [Particle<Self::TUnit>]) -> GBest<Self::TUnit> {
        for p in particles.iter_mut() {
            evaluate(&self.fit_calc, &mut self.gbest, p);
            self.tu1.update(&self.gbest, p.as_mut());
            self.tu2.update(&self.gbest, p.as_mut());
        }

        self.gbest
    }
}

fn evaluate<TFit: FitCalc>(fit_calc: &TFit, gbest: &mut GBest<TFit::T>, p: &mut Particle<TFit::T>) {
    p.fit = fit_calc.calculate_fit(p.pos);

    if p.fit > p.best_fit {
        p.best_fit = p.fit;
        p.best_pos = p.pos;
    }

    if p.fit > gbest.best_fit {
        gbest.best_fit = p.fit;
        gbest.best_pos = p.pos;
    }
}

/// The standard global-best PSO velocity update. Ignores context.
#[derive(Clone, Debug)]
pub struct GBestSinglePMover<TUnit, TRng, TContext> {
    config: SearchConfigGBest,
    rng: TRng,
    phantom: PhantomData<fn() -> (TUnit, TContext)>,
}

impl<TUnit, TRng, TContext> GBestSinglePMover<TUnit, TRng, TContext>
where
    TUnit: Unit,
    TRng: Rng,
    TContext: Copy,
{
    pub fn new(config: SearchConfigGBest, rng: TRng) -> Self {
        GBestSinglePMover {
            config,
            rng,
            phantom: PhantomData,
        }
    }
}

impl<TUnit, TRng, TContext: Copy> Contextful for GBestSinglePMover<TUnit, TRng, TContext> {
    type TContext = TContext;

    fn set_context(&mut self, _context: Self::TContext) {}
}

impl<TUnit, TRng, TContext> Mover for GBestSinglePMover<TUnit, TRng, TContext>
where
    TUnit: Unit,
    TRng: Rng,
    TContext: Copy,
{
    type TUnit = TUnit;
    type TCommon = GBest<TUnit>;

    fn update(&mut self, common: &Self::TCommon, p: ParticleMut<'_, Self::TUnit>) {
        let rng = &mut self.rng;
        let (c1, c2) = (self.config.cognitive_coeff, self.config.social_coeff);
        let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * c1 * rng.random::<f64>());
        let social = (common.best_pos - *p.pos).map_scalars(|d| d * c2 * rng.random::<f64>());

        *p.vel = *p.vel * self.config.inertia + cognitive + social;
        *p.pos = *p.pos + *p.vel;
    }
}
