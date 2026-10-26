//! V11: inside an inner search, the fitness, the boundary and the mover may
//! each use the outer particle or ignore it. Every combination composes with
//! no annotations, and each context-using component receives the context.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use hidden_common::*;
use nestswarm::{
    Boundary, BoundedMover, Contextful, FitCalc, GBest, GBestSearcherOne, GBestSearcherTwo,
    GBestSinglePMover, Mover, NestedMover, OnPart, ParticleMut, SearcherIter,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// A fitness for `t` that doesn't look at the outer particle.
#[derive(Clone, Debug)]
struct PlainFit;
impl Contextful for PlainFit {
    type TContext = W;
    fn set_context(&mut self, _: W) {}
}
impl FitCalc for PlainFit {
    type T = T;
    fn calculate_fit(&self, t: T) -> f64 {
        -sq_dist(t.0, [0.0; K])
    }
}

/// A boundary for `t` that doesn't look at the outer particle.
#[derive(Clone, Debug)]
struct PlainBounds;
impl Contextful for PlainBounds {
    type TContext = W;
    fn set_context(&mut self, _: W) {}
}
impl Boundary for PlainBounds {
    type T = T;
    fn handle(&self, t: T) -> T {
        T(clamp(t.0, [-20.0; K], [20.0; K]))
    }
}

/// A mover that uses the outer particle: it nudges `t` towards the outer `x`.
#[derive(Clone, Debug)]
struct PullToX(W);
impl Contextful for PullToX {
    type TContext = W;
    fn set_context(&mut self, w: W) {
        self.0 = w;
    }
}
impl Mover for PullToX {
    type TUnit = T;
    type TCommon = GBest<T>;
    fn update(&mut self, _: &GBest<T>, p: ParticleMut<'_, T>) {
        *p.pos = *p.pos + (T(self.0.x.0) - *p.pos) * 0.01;
    }
}

macro_rules! run {
    ($fit:expr, $bound:expr, $mover:expr, $log:expr) => {{
        let problem = Problem::standard();
        let inner = GBestSearcherOne::new($fit, BoundedMover::new($mover, $bound));
        let mut outer = GBestSearcherTwo::new(
            OuterFit(problem),
            BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(1)), XBounds(problem))
                .on_part::<X>(),
            NestedMover::new(3, inner, RecInit { inner: TInit { problem, count: 2 }, log: $log.clone() }, StdRng::seed_from_u64(2)),
        );
        let mut rng = StdRng::seed_from_u64(3);
        let mut group = initial_group(&problem, 3, &mut rng);
        SearcherIter::new(2, &mut outer, &mut group).for_each(drop);
    }};
}

const PER_SEARCH: usize = 2 * 3;

#[test]
fn every_combination_of_context_use() {
    let p = Problem::standard();
    let pso = || GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(9));

    // fit, boundary, mover: C = uses context, N = ignores it.
    let l = Log::<W>::new();
    run!(PlainFit, PlainBounds, pso(), l); // NNN
    assert!(!l.events().is_empty());

    let l = Log::<W>::new();
    run!(RecFit::new(InnerFit::new(p), l.clone()), PlainBounds, pso(), l); // CNN
    check_counts(&l.events(), PER_SEARCH, &["fit"]);

    let l = Log::<W>::new();
    run!(PlainFit, RecBound::new(TBounds::new(p), l.clone()), pso(), l); // NCN
    check_counts(&l.events(), PER_SEARCH, &["bound"]);

    let l = Log::<W>::new();
    run!(PlainFit, PlainBounds, RecMover::new(PullToX(W::default()), l.clone()), l); // NNC
    check_counts(&l.events(), PER_SEARCH, &["move"]);

    let l = Log::<W>::new();
    run!(RecFit::new(InnerFit::new(p), l.clone()), RecBound::new(TBounds::new(p), l.clone()), pso(), l); // CCN
    check_counts(&l.events(), PER_SEARCH, &["fit", "bound"]);

    let l = Log::<W>::new();
    run!(RecFit::new(InnerFit::new(p), l.clone()), PlainBounds, RecMover::new(PullToX(W::default()), l.clone()), l); // CNC
    check_counts(&l.events(), PER_SEARCH, &["fit", "move"]);

    let l = Log::<W>::new();
    run!(PlainFit, RecBound::new(TBounds::new(p), l.clone()), RecMover::new(PullToX(W::default()), l.clone()), l); // NCC
    check_counts(&l.events(), PER_SEARCH, &["bound", "move"]);

    let l = Log::<W>::new();
    run!(
        RecFit::new(InnerFit::new(p), l.clone()),
        RecBound::new(TBounds::new(p), l.clone()),
        RecMover::new(PullToX(W::default()), l.clone()),
        l
    ); // CCC
    check_counts(&l.events(), PER_SEARCH, &["fit", "bound", "move"]);
}
