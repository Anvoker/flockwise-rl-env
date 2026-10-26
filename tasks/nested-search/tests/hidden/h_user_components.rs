//! A rule written by the user composes with the library's components at
//! every position, including inside field rules and nested searches.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use flockwise::*;
use hidden_common::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn user_rule_inside_confined_field_rule_and_nested_search() {
    let problem = Problem::standard();
    let (x_star, t_star) = problem.optimum();

    for seed in seed_base()..seed_base() + 3 {
        let inner = Swarm1::new(
            objective(move |w| inner_fit(problem, w)),
            Confined::new(rule(|_| constricted_pso::<T>()), region(move |w| t_bounds(problem, w))),
        );

        let mut outer = Swarm2::new(
            objective(move |_| outer_fit(problem)),
            Confined::new(rule(|_| constricted_pso::<X>()), region(move |_| x_bounds(problem))).for_field::<X>(),
            InnerSearch::new(30, inner, seeder(t_init(problem, 10))),
        );

        let mut rng = StdRng::seed_from_u64(seed * 10 + 3);
        let mut group = initial_group(&problem, 20, &mut rng);
        let best = Run::new(60, &mut outer, &mut group, seed).last().unwrap().best_pos;
        let dx = max_abs_diff(best.x.0, x_star);
        let dt = max_abs_diff(best.t.0, t_star);
        assert!(dx < 0.05 && dt < 0.05, "seed {seed}: {best:?}, dx {dx}, dt {dt}");
    }
}

#[test]
fn user_rule_on_a_field_without_a_region() {
    // A field rule directly around a user rule, and an inner level built only
    // from user components.
    let problem = Problem::standard();
    let log = Log::<W>::new();

    let (l1, l2) = (log.clone(), log.clone());
    let inner = Swarm1::new(
        objective(move |w: W| rec_fit(l1.clone(), w, inner_fit(problem, w))),
        rule(move |w: W| rec_move(l2.clone(), w, constricted_pso::<T>())),
    );

    let mut outer = Swarm2::new(
        objective(move |_| outer_fit(problem)),
        rule(|_| constricted_pso::<X>()).for_field::<X>(),
        InnerSearch::new(4, inner, seeder(rec_init(log.clone(), t_init(problem, 3)))),
    );

    let mut rng = StdRng::seed_from_u64(4);
    let mut group = initial_group(&problem, 5, &mut rng);
    Run::new(3, &mut outer, &mut group, 0).for_each(drop);
    check_counts(&log.events(), 3 * 4, &["fit", "move"]);
}

#[test]
fn a_user_rule_uses_the_outer_position_and_the_field_of_the_best() {
    // Inside an inner search over `t`, a user rule moves each particle
    // halfway to the best `t` so far, then halfway to the outer `x`. With
    // one particle per inner search, its result is fully determined.
    let problem = Problem::standard();
    let inner = Swarm1::new(
        objective(move |w| inner_fit(problem, w)),
        rule(|w: W| {
            move |best: &BestSoFar<T>, _: &mut dyn rand::RngCore, p: ParticleView<'_, T>| {
                *p.pos = (*p.pos + best.best_pos) * 0.5;
                *p.pos = *p.pos + (T(w.x.0) - *p.pos) * 0.5;
            }
        }),
    );
    let mut outer = Swarm1::new(
        objective(move |_| outer_fit(problem)),
        InnerSearch::new(2, inner, seeder(|w: W, _: &mut dyn rand::RngCore| vec![Particle::new(T(add(w.x.0, 1.0)), T::default())])),
    );

    let x = [1.0, 2.0, 3.0];
    let mut group = vec![Particle::new(W { x: X(x), t: T([0.0; K]) }, W::default())];
    Run::new(1, &mut outer, &mut group, 0).for_each(drop);

    // Iteration 1 scores the start, x + 1, which becomes the best so far,
    // and the rule moves it to x + 0.5. Iteration 2 scores x + 0.5, which is
    // better (the target is x + c, c = (0.5, 2.5, -3)), so it is the result.
    assert_eq!(group[0].pos.t, T(add(x, 0.5)));
}
