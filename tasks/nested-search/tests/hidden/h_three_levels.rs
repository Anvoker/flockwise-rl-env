//! Three levels of nesting, with recording components at depth.
//!
//! Positions: W = (x, yt), YT = (y, t).
//! Score: -|x - a|^2 - |y - (x + c1)|^2 - |t - (y + c2)|^2
//! Constraints: y within w1 of x, t within w2 of y.
//! Level 1 moves x, level 2 moves y for each level-1 particle, level 3 moves t
//! for each level-2 particle. Each level only sees the level directly above.
//! Optimum: x = a, y = a + clamp(c1, w1), t = y + clamp(c2, w2).

mod common;
mod hidden_common;

use common::*;
use flockwise::*;
use hidden_common::*;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

const K: usize = 2;

vector_unit!(X, K);
vector_unit!(Y, K);
vector_unit!(T, K);
pair_unit!(YT, y: Y, t: T);
pair_unit!(W, x: X, yt: YT);

#[derive(Copy, Clone, Debug)]
struct Chain {
    a: [f64; K],
    c1: [f64; K],
    c2: [f64; K],
    w1: f64,
    w2: f64,
}

impl Chain {
    fn y_term(&self, x: [f64; K], y: [f64; K]) -> f64 {
        -sq_dist(y, std::array::from_fn(|i| x[i] + self.c1[i]))
    }
    fn t_term(&self, y: [f64; K], t: [f64; K]) -> f64 {
        -sq_dist(t, std::array::from_fn(|i| y[i] + self.c2[i]))
    }
    fn optimum(&self) -> ([f64; K], [f64; K], [f64; K]) {
        let y: [f64; K] = std::array::from_fn(|i| self.a[i] + self.c1[i].clamp(-self.w1, self.w1));
        let t = std::array::from_fn(|i| y[i] + self.c2[i].clamp(-self.w2, self.w2));
        (self.a, y, t)
    }
}

// Level 1.
fn fit1(c: Chain) -> impl Fn(W) -> f64 + Clone + Send + Sync {
    move |w: W| -sq_dist(w.x.0, c.a) + c.y_term(w.x.0, w.yt.y.0) + c.t_term(w.yt.y.0, w.yt.t.0)
}

fn x_box(x: X) -> X {
    X(clamp(x.0, [-50.0; K], [50.0; K]))
}

// Level 2: sees the level-1 particle.
fn fit2(c: Chain, w: W) -> impl Fn(YT) -> f64 + Clone + Send + Sync {
    move |yt: YT| c.y_term(w.x.0, yt.y.0) + c.t_term(yt.y.0, yt.t.0)
}

fn y_bounds(c: Chain, w: W) -> impl Fn(Y) -> Y + Clone + Send + Sync {
    let (lo, hi) = (add(w.x.0, -c.w1), add(w.x.0, c.w1));
    move |y: Y| Y(clamp(y.0, lo, hi))
}

fn init2(c: Chain, count: usize) -> impl Fn(W, &mut dyn RngCore) -> Vec<Particle<YT>> + Clone + Send + Sync {
    move |w: W, rng: &mut dyn RngCore| {
        let x = w.x.0;
        (0..count)
            .map(|_| {
                let y = uniform(rng, add(x, -c.w1), add(x, c.w1));
                let t = uniform(rng, add(y, -c.w2), add(y, c.w2));
                let pos = YT { y: Y(y), t: T(t) };
                let vel = YT { y: Y(uniform(rng, [-0.1; K], [0.1; K])), t: T::default() };
                Particle::new(pos, vel)
            })
            .collect()
    }
}

// Level 3: sees the level-2 particle only.
fn fit3(c: Chain, yt: YT) -> impl Fn(T) -> f64 + Clone + Send + Sync {
    move |t: T| c.t_term(yt.y.0, t.0)
}

fn t_bounds(c: Chain, yt: YT) -> impl Fn(T) -> T + Clone + Send + Sync {
    let (lo, hi) = (add(yt.y.0, -c.w2), add(yt.y.0, c.w2));
    move |t: T| T(clamp(t.0, lo, hi))
}

fn init3(c: Chain, count: usize) -> impl Fn(YT, &mut dyn RngCore) -> Vec<Particle<T>> + Clone + Send + Sync {
    move |yt: YT, rng: &mut dyn RngCore| {
        let y = yt.y.0;
        (0..count)
            .map(|_| {
                let t = T(uniform(rng, add(y, -c.w2), add(y, c.w2)));
                Particle::new(t, T(uniform(rng, [-0.1; K], [0.1; K])))
            })
            .collect()
    }
}

#[derive(Clone)]
struct Logs {
    level2: Log<W>,
    level3: Log<YT>,
}

fn level3(chain: Chain, log: Log<YT>) -> impl Search<Pos = T, Outer = YT> + Clone + Send {
    let (l1, l2) = (log.clone(), log);
    Swarm1::new(
        objective(move |yt: YT| rec_fit(l1.clone(), yt, fit3(chain, yt))),
        Confined::new(PsoRule::new(CONFIG), region(move |yt: YT| rec_bound(l2.clone(), yt, t_bounds(chain, yt)))),
    )
}

fn level2(chain: Chain, logs: Logs, sizes: (usize, usize, usize, usize)) -> impl Search<Pos = YT, Outer = W> + Clone + Send {
    let (_, _, p3, n3) = sizes;
    let (l1, l2) = (logs.level2.clone(), logs.level2.clone());
    Swarm2::new(
        objective(move |w: W| rec_fit(l1.clone(), w, fit2(chain, w))),
        Confined::new(PsoRule::new(CONFIG), region(move |w: W| rec_bound(l2.clone(), w, y_bounds(chain, w)))).for_field::<Y>(),
        InnerSearch::new(n3, level3(chain, logs.level3.clone()), seeder(rec_init(logs.level3.clone(), init3(chain, p3)))),
    )
}

fn solve(chain: Chain, seed: u64, logs: &Logs, sizes: (usize, usize, usize, usize), outer: (usize, usize)) -> W {
    let (p2, n2, _, _) = sizes;
    let (p1, n1) = outer;

    let mut searcher = Swarm2::new(
        objective(move |_| fit1(chain)),
        Confined::new(PsoRule::new(CONFIG), region(|_| x_box)).for_field::<X>(),
        InnerSearch::new(n2, level2(chain, logs.clone(), sizes), seeder(rec_init(logs.level2.clone(), init2(chain, p2)))),
    );

    let mut rng = StdRng::seed_from_u64(seed + 30);
    let mut group: Vec<Particle<W>> = (0..p1)
        .map(|_| {
            let x = X(uniform(&mut rng, [-5.0; K], [5.0; K]));
            let pos = W { x, yt: YT { y: Y(x.0), t: T(x.0) } };
            let vel = W { x: X(uniform(&mut rng, [-1.0; K], [1.0; K])), yt: YT::default() };
            Particle::new(pos, vel)
        })
        .collect();

    Run::new(n1, &mut searcher, &mut group, seed).last().unwrap().best_pos
}

fn chain() -> Chain {
    Chain { a: [2.0, -3.0], c1: [1.5, -0.4], c2: [-2.0, 0.8], w1: 1.0, w2: 1.2 }
}

#[test]
fn three_levels_find_the_optimum() {
    let chain = chain();
    let (x_star, y_star, t_star) = chain.optimum();

    for seed in seed_base()..seed_base() + 3 {
        let logs = Logs { level2: Log::new(), level3: Log::new() };
        let best = solve(chain, seed * 100, &logs, (8, 15, 6, 15), (16, 30));
        let dx = max_abs_diff(best.x.0, x_star);
        let dy = max_abs_diff(best.yt.y.0, y_star);
        let dt = max_abs_diff(best.yt.t.0, t_star);
        assert!(dx < 0.05 && dy < 0.05 && dt < 0.05, "seed {seed}: {best:?} dx {dx} dy {dy} dt {dt}");
    }
}

#[test]
fn each_level_is_prepared_for_the_level_directly_above() {
    let chain = chain();
    let logs = Logs { level2: Log::new(), level3: Log::new() };
    let (p2, n2, p3, n3) = (3, 2, 2, 3);
    solve(chain, 7, &logs, (p2, n2, p3, n3), (3, 2));

    check_counts(&logs.level2.events(), p2 * n2, &["fit", "bound"]);
    check_counts(&logs.level3.events(), p3 * n3, &["fit", "bound"]);
    check_prepared_first(&logs.level2.events());
    check_prepared_first(&logs.level3.events());
}
