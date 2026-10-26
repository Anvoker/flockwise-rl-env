//! Other arrangements of the same components: a different position layout
//! (fields declared in the other order, five dimensions), and the inner
//! search placed first in the outer search.
//!
//! With the inner search first, the inner result is computed for the outer
//! particle's position before the outer rule moves it, so each scored pair
//! holds a slightly stale inner field and the search settles near, not at,
//! the optimum. For that order the test asserts a calibrated bound instead.

mod common;
mod hidden_common;

use common::*;
use flockwise::*;
use hidden_common::*;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

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

const PROB: Prob = Prob { a: [1.0, -1.0, 2.0, 0.5, -2.5], c: [2.0, -0.3, 0.0, -1.7, 0.6], w: 0.8 };

fn outer_score(prob: Prob) -> impl Fn(Pq) -> f64 + Clone + Send + Sync {
    move |v: Pq| prob.fitness(v.p.0, v.q.0)
}

fn p_box(p: P) -> P {
    P(clamp(p.0, [-40.0; K], [40.0; K]))
}

fn inner_q(prob: Prob, outer: Pq) -> impl Fn(Q) -> f64 + Clone + Send + Sync {
    move |q: Q| prob.fitness(outer.p.0, q.0)
}

fn q_bounds(prob: Prob, outer: Pq) -> impl Fn(Q) -> Q + Clone + Send + Sync {
    let (lo, hi) = (add(outer.p.0, -prob.w), add(outer.p.0, prob.w));
    move |q: Q| Q(clamp(q.0, lo, hi))
}

fn q_init(prob: Prob, count: usize) -> impl Fn(Pq, &mut dyn RngCore) -> Vec<Particle<Q>> + Clone + Send + Sync {
    move |v: Pq, rng: &mut dyn RngCore| {
        let p = v.p.0;
        (0..count)
            .map(|_| Particle::new(Q(uniform(rng, add(p, -prob.w), add(p, prob.w))), Q::default()))
            .collect()
    }
}

fn start(seed: u64, count: usize) -> Vec<Particle<Pq>> {
    let mut rng = StdRng::seed_from_u64(seed * 7 + 3);
    (0..count)
        .map(|_| {
            let p = P(uniform(&mut rng, [-4.0; K], [4.0; K]));
            let v = P(uniform(&mut rng, [-1.0; K], [1.0; K]));
            Particle::new(Pq { q: Q(p.0), p }, Pq { q: Q::default(), p: v })
        })
        .collect()
}

#[test]
fn reordered_fields_in_five_dimensions() {
    let p_star = PROB.a;
    let q_star: [f64; K] = std::array::from_fn(|i| PROB.a[i] + PROB.c[i].clamp(-PROB.w, PROB.w));

    for seed in seed_base()..seed_base() + 3 {
        let inner = Swarm1::new(
            objective(|v| inner_q(PROB, v)),
            Confined::new(PsoRule::new(CONFIG), region(|v| q_bounds(PROB, v))),
        );

        let mut outer = Swarm2::new(
            objective(|_| outer_score(PROB)),
            Confined::new(PsoRule::new(CONFIG), region(|_| p_box)).for_field::<P>(),
            InnerSearch::new(40, inner, seeder(q_init(PROB, 12))),
        );

        let mut group = start(seed, 25);
        let best = Run::new(80, &mut outer, &mut group, seed).last().unwrap().best_pos;
        let dp = max_abs_diff(best.p.0, p_star);
        let dq = max_abs_diff(best.q.0, q_star);
        assert!(dp < 0.05 && dq < 0.05, "seed {seed}: {best:?}, dp {dp}, dq {dq}");
    }
}

#[test]
fn inner_search_first_composes_and_prepares_its_components() {
    let log = Log::<Pq>::new();

    let (l1, l2) = (log.clone(), log.clone());
    let inner = Swarm1::new(
        objective(move |v: Pq| rec_fit(l1.clone(), v, inner_q(PROB, v))),
        Confined::new(PsoRule::new(CONFIG), region(move |v: Pq| rec_bound(l2.clone(), v, q_bounds(PROB, v)))),
    );

    let mut outer = Swarm2::new(
        objective(|_| outer_score(PROB)),
        InnerSearch::new(4, inner, seeder(rec_init(log.clone(), q_init(PROB, 3)))),
        Confined::new(PsoRule::new(CONFIG), region(|_| p_box)).for_field::<P>(),
    );

    let mut rng = StdRng::seed_from_u64(4);
    let mut group: Vec<Particle<Pq>> = (0..5)
        .map(|_| {
            let p = P(uniform(&mut rng, [-4.0; K], [4.0; K]));
            Particle::new(Pq { q: Q(p.0), p }, Pq::default())
        })
        .collect();

    Run::new(3, &mut outer, &mut group, 0).for_each(drop);
    check_counts(&log.events(), 3 * 4, &["fit", "bound"]);
    check_prepared_first(&log.events());
}

#[test]
fn inner_search_first_still_converges_near_the_optimum() {
    for seed in seed_base()..seed_base() + 3 {
        let inner = Swarm1::new(
            objective(|v| inner_q(PROB, v)),
            Confined::new(PsoRule::new(CONFIG), region(|v| q_bounds(PROB, v))),
        );
        let mut outer = Swarm2::new(
            objective(|_| outer_score(PROB)),
            InnerSearch::new(40, inner, seeder(q_init(PROB, 12))),
            Confined::new(PsoRule::new(CONFIG), region(|_| p_box)).for_field::<P>(),
        );

        let mut group = start(seed, 25);
        let start_best = group.iter().map(|g| PROB.fitness(g.pos.p.0, g.pos.q.0)).fold(f64::NEG_INFINITY, f64::max);

        let best = Run::new(80, &mut outer, &mut group, seed).last().unwrap();
        let dp = max_abs_diff(best.best_pos.p.0, PROB.a);
        assert!(start_best < OPTIMUM - 5.0, "seed {seed}: the start should be far from the optimum");
        assert!(best.best_score > OPTIMUM - 1.0, "seed {seed}: score {} vs optimum {OPTIMUM}", best.best_score);
        assert!(dp < 2.0, "seed {seed}: dp {dp}");
    }
}

/// The constrained optimum's score: -sum(max(|c| - w, 0)^2) = -(1.2^2 + 0.9^2).
/// Calibrated over 900 runs of the reference: the final gap to it was at most
/// 0.37 and the distance at most 1.17, while the starting gap was at least 5.4.
const OPTIMUM: f64 = -2.25;
