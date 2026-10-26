//! V4: other arrangements of the same components: a different unit layout
//! (parts declared in the other order, five dimensions), and the nested mover
//! placed first in the outer searcher.
//!
//! With the nested mover first, the inner result is computed for the outer
//! particle's position before the outer mover moves it, so each evaluated pair
//! holds a slightly stale inner part and the search settles near, not at, the
//! optimum. For that order the test asserts a calibrated bound instead.

mod common;
mod hidden_common;

use hidden_common::*;

use common::*;
use nestswarm::{
    Boundary, BoundedMover, Contextful, FitCalc, GBestSearcherOne, GBestSearcherTwo,
    GBestSinglePMover, NestedInit, NestedMover, OnPart, Particle, SearcherIter,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const K: usize = 5;

vector_unit!(P, K);
vector_unit!(Q, K);
pair_unit!(Pq, q: Q, p: P);

/// -|p - a|^2 - |q - (p + c)|^2, q within w of p. The nested search moves q.
#[derive(Copy, Clone, Debug)]
struct Prob {
    a: [f64; K],
    c: [f64; K],
    w: f64,
}

impl Prob {
    fn fitness(&self, p: [f64; K], q: [f64; K]) -> f64 {
        -sq_dist(p, self.a) - sq_dist(q, std::array::from_fn(|i| p[i] + self.c[i]))
    }
}

#[derive(Clone, Debug)]
struct Outer(Prob);
impl Contextful for Outer {
    type TContext = ();
    fn set_context(&mut self, _: ()) {}
}
impl FitCalc for Outer {
    type T = Pq;
    fn calculate_fit(&self, v: Pq) -> f64 {
        self.0.fitness(v.p.0, v.q.0)
    }
}

#[derive(Clone, Debug)]
struct PBox;
impl Contextful for PBox {
    type TContext = ();
    fn set_context(&mut self, _: ()) {}
}
impl Boundary for PBox {
    type T = P;
    fn handle(&self, p: P) -> P {
        P(clamp(p.0, [-40.0; K], [40.0; K]))
    }
}

#[derive(Clone, Debug)]
struct InnerQ(Prob, Pq);
impl Contextful for InnerQ {
    type TContext = Pq;
    fn set_context(&mut self, v: Pq) {
        self.1 = v;
    }
}
impl FitCalc for InnerQ {
    type T = Q;
    fn calculate_fit(&self, q: Q) -> f64 {
        self.0.fitness(self.1.p.0, q.0)
    }
}

#[derive(Clone, Debug)]
struct QBounds(Prob, Pq);
impl Contextful for QBounds {
    type TContext = Pq;
    fn set_context(&mut self, v: Pq) {
        self.1 = v;
    }
}
impl Boundary for QBounds {
    type T = Q;
    fn handle(&self, q: Q) -> Q {
        let p = self.1.p.0;
        Q(clamp(q.0, add(p, -self.0.w), add(p, self.0.w)))
    }
}

#[derive(Clone, Debug)]
struct QInit(Prob, usize);
impl NestedInit for QInit {
    type TOuter = Pq;
    type TInner = Q;
    fn init<R: Rng + ?Sized>(&self, v: Pq, rng: &mut R) -> Vec<Particle<Q>> {
        let p = v.p.0;
        (0..self.1)
            .map(|_| Particle::new(Q(uniform(rng, add(p, -self.0.w), add(p, self.0.w))), Q::default()))
            .collect()
    }
}

#[test]
fn reordered_parts_in_five_dimensions() {
    let prob = Prob { a: [1.0, -1.0, 2.0, 0.5, -2.5], c: [2.0, -0.3, 0.0, -1.7, 0.6], w: 0.8 };
    let p_star = prob.a;
    let q_star: [f64; K] = std::array::from_fn(|i| prob.a[i] + prob.c[i].clamp(-prob.w, prob.w));

    for seed in seed_base()..seed_base() + 3 {
        let inner = GBestSearcherOne::new(
            InnerQ(prob, Pq::default()),
            BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed * 7)), QBounds(prob, Pq::default())),
        );

        let mut outer = GBestSearcherTwo::new(
            Outer(prob),
            BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed * 7 + 2)), PBox).on_part::<P>(),
            NestedMover::new(40, inner, QInit(prob, 12), StdRng::seed_from_u64(seed * 7 + 1)),
        );

        let mut rng = StdRng::seed_from_u64(seed * 7 + 3);
        let mut group: Vec<Particle<Pq>> = (0..25)
            .map(|_| {
                let p = P(uniform(&mut rng, [-4.0; K], [4.0; K]));
                let v = P(uniform(&mut rng, [-1.0; K], [1.0; K]));
                Particle::new(Pq { q: Q(p.0), p }, Pq { q: Q::default(), p: v })
            })
            .collect();

        let best = SearcherIter::new(80, &mut outer, &mut group).last().unwrap().best_pos;
        let dp = max_abs_diff(best.p.0, p_star);
        let dq = max_abs_diff(best.q.0, q_star);
        assert!(dp < 0.05 && dq < 0.05, "seed {seed}: {best:?}, dp {dp}, dq {dq}");
    }
}

#[test]
fn nested_mover_first_composes_and_delivers_context() {
    let prob = Prob { a: [1.0, -1.0, 2.0, 0.5, -2.5], c: [2.0, -0.3, 0.0, -1.7, 0.6], w: 0.8 };
    let log = Log::<Pq>::new();

    let inner = GBestSearcherOne::new(
        RecFit::new(InnerQ(prob, Pq::default()), log.clone()),
        BoundedMover::new(
            GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(1)),
            RecBound::new(QBounds(prob, Pq::default()), log.clone()),
        ),
    );

    let mut outer = GBestSearcherTwo::new(
        Outer(prob),
        NestedMover::new(4, inner, RecInit { inner: QInit(prob, 3), log: log.clone() }, StdRng::seed_from_u64(2)),
        BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(3)), PBox).on_part::<P>(),
    );

    let mut rng = StdRng::seed_from_u64(4);
    let mut group: Vec<Particle<Pq>> = (0..5)
        .map(|_| {
            let p = P(uniform(&mut rng, [-4.0; K], [4.0; K]));
            Particle::new(Pq { q: Q(p.0), p }, Pq::default())
        })
        .collect();

    SearcherIter::new(3, &mut outer, &mut group).for_each(drop);
    check_counts(&log.events(), 3 * 4, &["fit", "bound"]);
}

#[test]
fn nested_mover_first_still_converges_near_the_optimum() {
    let prob = Prob { a: [1.0, -1.0, 2.0, 0.5, -2.5], c: [2.0, -0.3, 0.0, -1.7, 0.6], w: 0.8 };

    for seed in seed_base()..seed_base() + 3 {
        let inner = GBestSearcherOne::new(
            InnerQ(prob, Pq::default()),
            BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed * 7)), QBounds(prob, Pq::default())),
        );
        let mut outer = GBestSearcherTwo::new(
            Outer(prob),
            NestedMover::new(40, inner, QInit(prob, 12), StdRng::seed_from_u64(seed * 7 + 1)),
            BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed * 7 + 2)), PBox).on_part::<P>(),
        );

        let mut rng = StdRng::seed_from_u64(seed * 7 + 3);
        let mut group: Vec<Particle<Pq>> = (0..25)
            .map(|_| {
                let p = P(uniform(&mut rng, [-4.0; K], [4.0; K]));
                let v = P(uniform(&mut rng, [-1.0; K], [1.0; K]));
                Particle::new(Pq { q: Q(p.0), p }, Pq { q: Q::default(), p: v })
            })
            .collect();
        let start_best = group.iter().map(|g| prob.fitness(g.pos.p.0, g.pos.q.0)).fold(f64::NEG_INFINITY, f64::max);

        let best = SearcherIter::new(80, &mut outer, &mut group).last().unwrap();
        let dp = max_abs_diff(best.best_pos.p.0, prob.a);
        assert!(start_best < OPTIMUM - 5.0, "seed {seed}: the start should be far from the optimum");
        assert!(best.best_fit > OPTIMUM - 1.0, "seed {seed}: fitness {} vs optimum {OPTIMUM}", best.best_fit);
        assert!(dp < 2.0, "seed {seed}: dp {dp}");
    }
}

/// The constrained optimum's fitness: -sum(max(|c| - w, 0)^2) = -(1.2^2 + 0.9^2).
/// Calibrated over 900 runs of the reference: the final gap to it was at most
/// 0.37 and the distance at most 1.17, while the starting gap was at least 5.4.
const OPTIMUM: f64 = -2.25;
