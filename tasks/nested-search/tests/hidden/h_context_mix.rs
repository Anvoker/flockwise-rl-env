//! V11: inside an inner search, the fitness, the boundary and the mover may
//! each use the outer particle or ignore it. Every combination composes with
//! no annotations, and each context-using component receives the context.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use hidden_common::*;
use flockwise::{
    Region, Confined, OuterAware, Objective, BestSoFar, Swarm1, Swarm2,
    PsoRule, Rule, InnerSearch, ForField, ParticleView, Run,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// A fitness for `t` that doesn't look at the outer particle.
#[derive(Clone, Debug)]
struct PlainFit;
impl OuterAware for PlainFit {
    type Outer = W;
    fn see_outer(&mut self, _: W) {}
}
impl Objective for PlainFit {
    type Pos = T;
    fn score(&self, t: T) -> f64 {
        -sq_dist(t.0, [0.0; K])
    }
}

/// A boundary for `t` that doesn't look at the outer particle.
#[derive(Clone, Debug)]
struct PlainBounds;
impl OuterAware for PlainBounds {
    type Outer = W;
    fn see_outer(&mut self, _: W) {}
}
impl Region for PlainBounds {
    type Pos = T;
    fn confine(&self, t: T) -> T {
        T(clamp(t.0, [-20.0; K], [20.0; K]))
    }
}

/// A mover that uses the outer particle: it nudges `t` towards the outer `x`.
#[derive(Clone, Debug)]
struct PullToX(W);
impl OuterAware for PullToX {
    type Outer = W;
    fn see_outer(&mut self, w: W) {
        self.0 = w;
    }
}
impl Rule for PullToX {
    type Pos = T;
    type Shared = BestSoFar<T>;
    fn apply(&mut self, _: &BestSoFar<T>, p: ParticleView<'_, T>) {
        *p.pos = *p.pos + (T(self.0.x.0) - *p.pos) * 0.01;
    }
}

macro_rules! run {
    ($fit:expr, $bound:expr, $mover:expr, $log:expr) => {{
        let problem = Problem::standard();
        let inner = Swarm1::new($fit, Confined::new($mover, $bound));
        let mut outer = Swarm2::new(
            OuterFit(problem),
            Confined::new(PsoRule::new(CONFIG, StdRng::seed_from_u64(1)), XBounds(problem))
                .for_field::<X>(),
            InnerSearch::new(3, inner, RecInit { inner: TInit { problem, count: 2 }, log: $log.clone() }, StdRng::seed_from_u64(2)),
        );
        let mut rng = StdRng::seed_from_u64(3);
        let mut group = initial_group(&problem, 3, &mut rng);
        Run::new(2, &mut outer, &mut group).for_each(drop);
    }};
}

const PER_SEARCH: usize = 2 * 3;

#[test]
fn every_combination_of_context_use() {
    let p = Problem::standard();
    let pso = || PsoRule::new(CONFIG, StdRng::seed_from_u64(9));

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
