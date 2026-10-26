//! The same problem as `nested.rs` with the roles swapped: the outer search
//! moves `t`, and a nested search over `x` runs for every outer particle.

mod common;

use common::two_part::*;
use common::*;
use flockwise::{
    Region, Confined, OuterAware, Objective, Swarm1, Swarm2,
    PsoRule, Seeder, InnerSearch, ForField, Particle, Run,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Keeps the outer `t` in the same box as `x`.
#[derive(Clone, Debug)]
struct TBox;

impl OuterAware for TBox {
    type Outer = ();
    fn see_outer(&mut self, _: ()) {}
}

impl Region for TBox {
    type Pos = T;
    fn confine(&self, t: T) -> T {
        T(clamp(t.0, [-30.0; K], [30.0; K]))
    }
}

/// Scores `x` for the outer particle's `t`.
#[derive(Clone, Debug)]
struct InnerFitX {
    problem: Problem,
    outer: W,
}

impl OuterAware for InnerFitX {
    type Outer = W;
    fn see_outer(&mut self, outer: W) {
        self.outer = outer;
    }
}

impl Objective for InnerFitX {
    type Pos = X;
    fn score(&self, x: X) -> f64 {
        self.problem.fitness(x.0, self.outer.t.0)
    }
}

/// Keeps `x` within `w` of the outer particle's `t`, and inside the box.
#[derive(Clone, Debug)]
struct XBoundsFromT {
    problem: Problem,
    outer: W,
}

impl OuterAware for XBoundsFromT {
    type Outer = W;
    fn see_outer(&mut self, outer: W) {
        self.outer = outer;
    }
}

impl Region for XBoundsFromT {
    type Pos = X;
    fn confine(&self, x: X) -> X {
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

impl Seeder for XInit {
    type Outer = W;
    type Inner = X;

    fn seed<R: Rng + ?Sized>(&self, outer: W, rng: &mut R) -> Vec<Particle<X>> {
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
    let inner = Swarm1::new(
        InnerFitX {
            problem,
            outer: W::default(),
        },
        Confined::new(
            PsoRule::new(CONFIG),
            XBoundsFromT {
                problem,
                outer: W::default(),
            },
        ),
    );

    let mut outer = Swarm2::new(
        OuterFit(problem),
        Confined::new(PsoRule::new(CONFIG), TBox)
            .for_field::<T>(),
        InnerSearch::new(30, inner, XInit { problem, count: 10 }),
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut group: Vec<Particle<W>> = (0..20)
        .map(|_| {
            let t = T(uniform(&mut rng, [-6.0; K], [6.0; K]));
            let v = T(uniform(&mut rng, [-1.0; K], [1.0; K]));
            Particle::new(W { x: X(t.0), t }, W { x: X::default(), t: v })
        })
        .collect();

    Run::new(60, &mut outer, &mut group, seed).last().unwrap().best_pos
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
