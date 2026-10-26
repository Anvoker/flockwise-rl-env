//! The same problem as `nested.rs` with the roles swapped: the outer search
//! moves `t`, and a nested search over `x` runs for every outer particle.

mod common;

use common::two_part::*;
use common::*;
use nestswarm::{
    Boundary, BoundedMover, Contextful, FitCalc, GBestSearcherOne, GBestSearcherTwo,
    GBestSinglePMover, NestedInit, NestedMover, OnPart, Particle, SearcherIter,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Keeps the outer `t` in the same box as `x`.
#[derive(Clone, Debug)]
struct TBox;

impl Contextful for TBox {
    type TContext = ();
    fn set_context(&mut self, _: ()) {}
}

impl Boundary for TBox {
    type T = T;
    fn handle(&self, t: T) -> T {
        T(clamp(t.0, [-30.0; K], [30.0; K]))
    }
}

/// Scores `x` for the outer particle's `t`.
#[derive(Clone, Debug)]
struct InnerFitX {
    problem: Problem,
    outer: W,
}

impl Contextful for InnerFitX {
    type TContext = W;
    fn set_context(&mut self, outer: W) {
        self.outer = outer;
    }
}

impl FitCalc for InnerFitX {
    type T = X;
    fn calculate_fit(&self, x: X) -> f64 {
        self.problem.fitness(x.0, self.outer.t.0)
    }
}

/// Keeps `x` within `w` of the outer particle's `t`, and inside the box.
#[derive(Clone, Debug)]
struct XBoundsFromT {
    problem: Problem,
    outer: W,
}

impl Contextful for XBoundsFromT {
    type TContext = W;
    fn set_context(&mut self, outer: W) {
        self.outer = outer;
    }
}

impl Boundary for XBoundsFromT {
    type T = X;
    fn handle(&self, x: X) -> X {
        let t = self.outer.t.0;
        let w = self.problem.w;
        let lo = std::array::from_fn(|i| (t[i] - w).max(self.problem.x_lo[i]));
        let hi = std::array::from_fn(|i| (t[i] + w).min(self.problem.x_hi[i]));
        X(clamp(x.0, lo, hi))
    }
}

#[derive(Clone, Debug)]
struct XInit {
    problem: Problem,
    count: usize,
}

impl NestedInit for XInit {
    type TOuter = W;
    type TInner = X;

    fn init<R: Rng + ?Sized>(&self, outer: W, rng: &mut R) -> Vec<Particle<X>> {
        let t = outer.t.0;
        let w = self.problem.w;
        (0..self.count)
            .map(|_| {
                let x = X(uniform(rng, add(t, -w), add(t, w)));
                let v = X(uniform(rng, [-w; K], [w; K])) * 0.1;
                Particle::new(x, v)
            })
            .collect()
    }
}

fn solve(problem: Problem, seed: u64) -> W {
    let inner = GBestSearcherOne::new(
        InnerFitX {
            problem,
            outer: W::default(),
        },
        BoundedMover::new(
            GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed + 1)),
            XBoundsFromT {
                problem,
                outer: W::default(),
            },
        ),
    );

    let mut outer = GBestSearcherTwo::new(
        OuterFit(problem),
        BoundedMover::new(GBestSinglePMover::new(CONFIG, StdRng::seed_from_u64(seed + 2)), TBox)
            .on_part::<T>(),
        NestedMover::new(30, inner, XInit { problem, count: 10 }, StdRng::seed_from_u64(seed + 3)),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut group: Vec<Particle<W>> = (0..20)
        .map(|_| {
            let t = T(uniform(&mut rng, [-6.0; K], [6.0; K]));
            let v = T(uniform(&mut rng, [-1.0; K], [1.0; K]));
            Particle::new(W { x: X(t.0), t }, W { x: X::default(), t: v })
        })
        .collect();

    SearcherIter::new(60, &mut outer, &mut group).last().unwrap().best_pos
}

#[test]
fn swapped_roles_find_the_joint_optimum() {
    let problem = Problem::standard();
    let (x_star, t_star) = problem.optimum();

    for seed in seed_base()..seed_base() + 5 {
        let best = solve(problem, seed * 10);
        let dx = max_abs_diff(best.x.0, x_star);
        let dt = max_abs_diff(best.t.0, t_star);
        assert!(dx < 0.05 && dt < 0.05, "seed {seed}: {best:?}, dx {dx}, dt {dt}");
    }
}
