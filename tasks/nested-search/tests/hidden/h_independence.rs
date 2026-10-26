//! Every search starts fresh. A result found for one outer particle, or in
//! an earlier run, must never leak into another search.

mod common;

use common::two_part::*;
use common::*;
use flockwise::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::sync::{Arc, Mutex};

#[test]
fn inner_search_for_b_does_not_reuse_the_result_for_a() {
    // A sits at the optimum, so its inner search reaches a score near 0.
    // B is far away: everything reachable for B scores below -40. A stale or
    // shared best so far would hand B the timings found for A.
    let problem = Problem::standard();
    let (x_star, _) = problem.optimum();
    let x_b: [f64; K] = add(x_star, 4.0).map(|v| v.min(problem.x_hi[0]));

    for seed in seed_base()..seed_base() + 3 {
        let inner = Swarm1::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
        );
        // The inner search is the outer search's only rule, so the outer
        // particles' `x` never moves.
        let mut outer = Swarm1::new(
            objective(move |_| outer_fit(problem)),
            InnerSearch::new(100, inner, seeder(t_init(problem, 40))),
        );

        for order in [[x_star, x_b], [x_b, x_star]] {
            let mut group: Vec<Particle<W>> =
                order.iter().map(|&x| Particle::new(W { x: X(x), t: T(x) }, W::default())).collect();
            Run::new(1, &mut outer, &mut group, seed).for_each(drop);

            for p in &group {
                let x = p.pos.x.0;
                let want: [f64; K] = std::array::from_fn(|i| x[i] + problem.c[i].clamp(-problem.w, problem.w));
                assert!(max_abs_diff(p.pos.t.0, want) < 0.05, "seed {seed}: x {x:?} got t {:?}, want {want:?}", p.pos.t);
            }
            assert_eq!(group[0].pos.x.0, order[0], "the inner search only writes its own field");
            assert_eq!(group[1].pos.x.0, order[1], "the inner search only writes its own field");
        }
    }
}

#[test]
fn repeated_runs_of_one_searcher_start_fresh() {
    let center = Arc::new(Mutex::new([1.0, 1.0, 1.0]));
    // A sphere whose center can be changed between runs. Its closure reads
    // the center when the search begins.
    let c = center.clone();
    let mut searcher = Swarm1::new(
        objective(move |_| {
            let center = *c.lock().unwrap();
            move |x: X| -sq_dist(x.0, center)
        }),
        PsoRule::new(CONFIG),
    );

    let mut rng = StdRng::seed_from_u64(2);
    let group = |lo: f64, hi: f64, rng: &mut StdRng| -> Vec<Particle<X>> {
        (0..15)
            .map(|_| Particle::new(X(uniform(rng, [lo; K], [hi; K])), X::default()))
            .collect()
    };

    let mut g1 = group(0.0, 2.0, &mut rng);
    let first = Run::new(40, &mut searcher, &mut g1, 0).last().unwrap();
    assert!(max_abs_diff(first.best_pos.0, [1.0; K]) < 1e-2);

    // Move the target far away. The old best now scores about -300, but its
    // stored score from the first run is near 0.
    *center.lock().unwrap() = [-9.0, -9.0, -9.0];
    let mut g2 = group(-10.0, -8.0, &mut rng);
    let second = Run::new(40, &mut searcher, &mut g2, 0).last().unwrap();
    assert!(
        max_abs_diff(second.best_pos.0, [-9.0; K]) < 1e-2,
        "second run returned {:?}",
        second.best_pos
    );
}

#[test]
fn best_so_far_after_one_iteration_is_the_best_starting_position() {
    let problem = Problem::standard();
    let a = problem.a;
    let mut searcher = Swarm1::new(objective(move |_| move |x: X| -sq_dist(x.0, a)), PsoRule::new(CONFIG));
    let mut rng = StdRng::seed_from_u64(4);
    let mut group: Vec<Particle<X>> = (0..10)
        .map(|_| Particle::new(X(uniform(&mut rng, [-6.0; K], [6.0; K])), X::default()))
        .collect();
    let initial: Vec<X> = group.iter().map(|p| p.pos).collect();

    let best = Run::new(1, &mut searcher, &mut group, 0).next().unwrap();
    let want = initial
        .iter()
        .copied()
        .max_by(|p, q| (-sq_dist(p.0, a)).total_cmp(&-sq_dist(q.0, a)))
        .unwrap();
    assert_eq!(best.best_pos, want);
}
