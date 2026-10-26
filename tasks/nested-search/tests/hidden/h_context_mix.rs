//! Inside an inner search, the objective, the region and the rule may each
//! use the outer particle or ignore it. Every combination composes with no
//! annotations, and each component that uses the outer position is prepared
//! once per search with it, and called with it.

mod common;
mod hidden_common;

use common::two_part::*;
use common::*;
use flockwise::*;
use hidden_common::*;
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// A rule that uses the outer particle: it nudges `t` towards the outer `x`.
fn pull_to_x(w: W) -> impl Fn(&BestSoFar<T>, &mut dyn RngCore, ParticleView<'_, T>) + Clone + Send + Sync {
    move |_: &BestSoFar<T>, _: &mut dyn RngCore, p: ParticleView<'_, T>| {
        *p.pos = *p.pos + (T(w.x.0) - *p.pos) * 0.01;
    }
}

// The components, each either ignoring the outer position (N) or using it (C).
macro_rules! fit_n {
    () => {
        objective(|_| |t: T| -sq_dist(t.0, [0.0; K]))
    };
}
macro_rules! bound_n {
    () => {
        region(|_| |t: T| T(clamp(t.0, [-20.0; K], [20.0; K])))
    };
}
macro_rules! move_n {
    () => {
        PsoRule::new(CONFIG)
    };
}
macro_rules! fit_c {
    ($l:expr, $p:expr) => {{
        let (l, p) = ($l.clone(), $p);
        objective(move |w: W| rec_fit(l.clone(), w, inner_fit(p, w)))
    }};
}
macro_rules! bound_c {
    ($l:expr, $p:expr) => {{
        let (l, p) = ($l.clone(), $p);
        region(move |w: W| rec_bound(l.clone(), w, t_bounds(p, w)))
    }};
}
macro_rules! move_c {
    ($l:expr) => {{
        let l = $l.clone();
        rule(move |w: W| rec_move(l.clone(), w, pull_to_x(w)))
    }};
}

macro_rules! run {
    ($fit:expr, $bound:expr, $mover:expr, $log:expr) => {{
        let problem = Problem::standard();
        let inner = Swarm1::new($fit, Confined::new($mover, $bound));
        let mut outer = Swarm2::new(
            objective(move |_| outer_fit(problem)),
            Confined::new(PsoRule::new(CONFIG), region(move |_| x_bounds(problem))).for_field::<X>(),
            InnerSearch::new(3, inner, seeder(rec_init($log.clone(), t_init(problem, 2)))),
        );
        let mut rng = StdRng::seed_from_u64(3);
        let mut group = initial_group(&problem, 3, &mut rng);
        Run::new(2, &mut outer, &mut group, 0).for_each(drop);
    }};
}

const PER_SEARCH: usize = 2 * 3;

#[test]
fn every_combination_of_context_use() {
    let p = Problem::standard();

    // objective, region, rule: C = uses the outer position, N = ignores it.
    let l = Log::<W>::new();
    run!(fit_n!(), bound_n!(), move_n!(), l); // NNN
    assert!(!l.events().is_empty());

    let l = Log::<W>::new();
    run!(fit_c!(l, p), bound_n!(), move_n!(), l); // CNN
    check_counts(&l.events(), PER_SEARCH, &["fit"]);

    let l = Log::<W>::new();
    run!(fit_n!(), bound_c!(l, p), move_n!(), l); // NCN
    check_counts(&l.events(), PER_SEARCH, &["bound"]);

    let l = Log::<W>::new();
    run!(fit_n!(), bound_n!(), move_c!(l), l); // NNC
    check_counts(&l.events(), PER_SEARCH, &["move"]);

    let l = Log::<W>::new();
    run!(fit_c!(l, p), bound_c!(l, p), move_n!(), l); // CCN
    check_counts(&l.events(), PER_SEARCH, &["fit", "bound"]);

    let l = Log::<W>::new();
    run!(fit_c!(l, p), bound_n!(), move_c!(l), l); // CNC
    check_counts(&l.events(), PER_SEARCH, &["fit", "move"]);

    let l = Log::<W>::new();
    run!(fit_n!(), bound_c!(l, p), move_c!(l), l); // NCC
    check_counts(&l.events(), PER_SEARCH, &["bound", "move"]);

    let l = Log::<W>::new();
    run!(fit_c!(l, p), bound_c!(l, p), move_c!(l), l); // CCC
    check_counts(&l.events(), PER_SEARCH, &["fit", "bound", "move"]);
}
