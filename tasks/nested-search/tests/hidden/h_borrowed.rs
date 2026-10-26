//! Components may borrow data that outlives the run. Compositions built from
//! them are returned from functions, nested, cloned, run in parallel and sent
//! to scoped threads like any other, and give the same results as
//! compositions that own their data.

mod common;

use common::two_part::*;
use common::*;
use flockwise::*;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rayon::ThreadPoolBuilder;

/// Data owned by the caller: neither `Clone` nor `'static`.
struct Data {
    problem: Problem,
    // Large enough that copying it into every component would hurt.
    weights: Vec<f64>,
}

impl Data {
    fn new() -> Data {
        Data { problem: Problem::standard(), weights: vec![1.0; 1 << 16] }
    }

    /// The problem, read through the borrowed table.
    fn problem(&self) -> Problem {
        let mut p = self.problem;
        p.w *= self.weights[p.a.len()];
        p
    }
}

fn borrowed_inner(data: &Data) -> impl Search<Pos = T, Outer = W> + Clone + Send {
    Swarm1::new(
        objective(move |w| inner_fit(data.problem(), w)),
        Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(data.problem(), w))),
    )
}

fn borrowed_outer(data: &Data) -> impl Search<Pos = W, Outer = ()> + Clone + Send {
    Swarm2::new(
        objective(move |_| outer_fit(data.problem())),
        Confined::new(
            rule(move |_| {
                // A rule that borrows too: PSO's update, read from the data.
                let _ = data.weights.len();
                move |best: &BestSoFar<X>, rng: &mut dyn RngCore, p: ParticleView<'_, X>| {
                    pso_x(best, rng, p);
                }
            }),
            region(move |_| x_bounds(data.problem())),
        )
        .for_field::<X>(),
        InnerSearch::new(
            6,
            borrowed_inner(data),
            seeder(move |w: W, rng: &mut dyn RngCore| t_init(data.problem(), 4)(w, rng)),
        ),
    )
}

/// The same composition, owning copies of everything.
fn owned_outer(problem: Problem) -> impl Search<Pos = W, Outer = ()> + Clone + Send {
    let inner = Swarm1::new(
        objective(move |w| inner_fit(problem, w)),
        Confined::new(PsoRule::new(CONFIG), region(move |w| t_bounds(problem, w))),
    );
    Swarm2::new(
        objective(move |_| outer_fit(problem)),
        Confined::new(
            rule(|_| |best: &BestSoFar<X>, rng: &mut dyn RngCore, p: ParticleView<'_, X>| pso_x(best, rng, p)),
            region(move |_| x_bounds(problem)),
        )
        .for_field::<X>(),
        InnerSearch::new(6, inner, seeder(t_init(problem, 4))),
    )
}

/// A plain PSO step on `x`, written by the user.
fn pso_x(best: &BestSoFar<X>, rng: &mut dyn RngCore, p: ParticleView<'_, X>) {
    use rand::Rng;
    let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * 1.5 * rng.random::<f64>());
    let social = (best.best_pos - *p.pos).map_scalars(|d| d * 1.5 * rng.random::<f64>());
    *p.vel = *p.vel * 0.7 + cognitive + social;
    *p.pos = *p.pos + *p.vel;
}

fn run<S: Search<Pos = W, Outer = ()>>(search: &mut S, seed: u64) -> (Vec<BestSoFar<W>>, Vec<Particle<W>>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut group = initial_group(&Problem::standard(), 10, &mut rng);
    let bests = Run::new(5, search, &mut group, seed).collect();
    (bests, group)
}

#[test]
fn borrowed_and_owned_compositions_agree() {
    let data = Data::new();
    let seed = 2 + seed_base();
    let owned = run(&mut owned_outer(data.problem()), seed);
    let borrowed = run(&mut borrowed_outer(&data), seed);
    assert!(owned == borrowed, "borrowing the data changed the results");
}

#[test]
fn borrowed_compositions_run_in_pools_clone_and_move_to_scoped_threads() {
    let data = Data::new();
    let seed = 4 + seed_base();
    let search = borrowed_outer(&data);

    let in_one = ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| run(&mut search.clone(), seed));
    let in_four = ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(|| run(&mut search.clone(), seed));
    assert!(in_one == in_four, "a borrowed composition depends on the thread count");

    let mut moved = search.clone();
    let on_thread = std::thread::scope(|s| s.spawn(move || run(&mut moved, seed)).join().unwrap());
    assert!(on_thread == in_one, "a borrowed composition moved to a scoped thread gave other results");
}

/// A composition stored in a struct next to a reference to its data.
struct Planner<'a, S> {
    data: &'a Data,
    search: S,
}

#[test]
fn a_borrowed_composition_is_stored_and_converges() {
    let data = Data::new();
    let mut planner = Planner { data: &data, search: borrowed_outer(&data) };
    let mut rng = StdRng::seed_from_u64(1 + seed_base());
    let mut group = initial_group(&planner.data.problem(), 20, &mut rng);
    let best = Run::new(60, &mut planner.search, &mut group, 1).last().unwrap();
    let (x_star, t_star) = planner.data.problem().optimum();
    assert!(max_abs_diff(best.best_pos.x.0, x_star) < 0.05, "{best:?}");
    assert!(max_abs_diff(best.best_pos.t.0, t_star) < 0.05, "{best:?}");
}
