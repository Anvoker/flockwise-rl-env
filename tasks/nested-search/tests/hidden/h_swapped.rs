//! The same problem as `nested.rs` with the roles swapped: the outer search
//! moves `t`, and a nested search over `x` runs for every outer particle.

mod common;

use common::two_part::*;
use common::*;
use flockwise::*;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

/// Scores `x` for the outer particle's `t`.
fn inner_fit_x(problem: Problem, outer: W) -> impl Fn(X) -> f64 + Clone + Send + Sync {
    move |x: X| problem.fitness(x.0, outer.t.0)
}

/// Keeps `x` within `w` of the outer particle's `t`, and inside the box.
fn x_bounds_from_t(problem: Problem, outer: W) -> impl Fn(X) -> X + Clone + Send + Sync {
    let t = outer.t.0;
    let w = problem.w;
    let lo: [f64; K] = std::array::from_fn(|i| (t[i] - w).max(problem.x_lo[i]));
    let hi: [f64; K] = std::array::from_fn(|i| (t[i] + w).min(problem.x_hi[i]));
    move |x: X| X(clamp(x.0, lo, hi))
}

fn x_init(problem: Problem, count: usize) -> impl Fn(W, &mut dyn RngCore) -> Vec<Particle<X>> + Clone + Send + Sync {
    move |outer: W, rng: &mut dyn RngCore| {
        let t = outer.t.0;
        let w = problem.w;
        (0..count)
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
        objective(move |w| inner_fit_x(problem, w)),
        Confined::new(PsoRule::new(CONFIG), region(move |w| x_bounds_from_t(problem, w))),
    );

    let mut outer = Swarm2::new(
        objective(move |_| outer_fit(problem)),
        Confined::new(PsoRule::new(CONFIG), region(|_| |t: T| T(clamp(t.0, [-30.0; K], [30.0; K])))).for_field::<T>(),
        InnerSearch::new(30, inner, seeder(x_init(problem, 10))),
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
