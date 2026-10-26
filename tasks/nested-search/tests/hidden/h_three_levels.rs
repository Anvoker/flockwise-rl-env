//! V3 (with V12 at depth): three levels of nesting.
//!
//! Units: W = (x, yt), YT = (y, t).
//! Fitness: -|x - a|^2 - |y - (x + c1)|^2 - |t - (y + c2)|^2
//! Constraints: y within w1 of x, t within w2 of y.
//! Level 1 moves x, level 2 moves y for each level-1 particle, level 3 moves t
//! for each level-2 particle. Each level only sees the level directly above.
//! Optimum: x = a, y = a + clamp(c1, w1), t = y + clamp(c2, w2).

mod common;
mod hidden_common;

use common::*;
use hidden_common::*;
use flockwise::{
    Region, Confined, OuterAware, Objective, Swarm1, Swarm2,
    PsoRule, Seeder, InnerSearch, ForField, Particle, Search, Run,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

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

macro_rules! no_context {
    ($ty:ty) => {
        impl OuterAware for $ty {
            type Outer = ();
            fn see_outer(&mut self, _: ()) {}
        }
    };
}

// Level 1.
#[derive(Clone, Debug)]
struct Fit1(Chain);
no_context!(Fit1);
impl Objective for Fit1 {
    type Pos = W;
    fn score(&self, w: W) -> f64 {
        -sq_dist(w.x.0, self.0.a) + self.0.y_term(w.x.0, w.yt.y.0) + self.0.t_term(w.yt.y.0, w.yt.t.0)
    }
}

#[derive(Clone, Debug)]
struct XBox;
no_context!(XBox);
impl Region for XBox {
    type Pos = X;
    fn confine(&self, x: X) -> X {
        X(clamp(x.0, [-50.0; K], [50.0; K]))
    }
}

// Level 2: sees the level-1 particle.
#[derive(Clone, Debug)]
struct Fit2(Chain, W);
impl OuterAware for Fit2 {
    type Outer = W;
    fn see_outer(&mut self, w: W) {
        self.1 = w;
    }
}
impl Objective for Fit2 {
    type Pos = YT;
    fn score(&self, yt: YT) -> f64 {
        self.0.y_term(self.1.x.0, yt.y.0) + self.0.t_term(yt.y.0, yt.t.0)
    }
}

#[derive(Clone, Debug)]
struct YBounds(Chain, W);
impl OuterAware for YBounds {
    type Outer = W;
    fn see_outer(&mut self, w: W) {
        self.1 = w;
    }
}
impl Region for YBounds {
    type Pos = Y;
    fn confine(&self, y: Y) -> Y {
        let x = self.1.x.0;
        Y(clamp(y.0, add(x, -self.0.w1), add(x, self.0.w1)))
    }
}

#[derive(Clone, Debug)]
struct Init2(Chain, usize);
impl Seeder for Init2 {
    type Outer = W;
    type Inner = YT;
    fn seed<R: Rng + ?Sized>(&self, w: W, rng: &mut R) -> Vec<Particle<YT>> {
        let x = w.x.0;
        (0..self.1)
            .map(|_| {
                let y = uniform(rng, add(x, -self.0.w1), add(x, self.0.w1));
                let t = uniform(rng, add(y, -self.0.w2), add(y, self.0.w2));
                let pos = YT { y: Y(y), t: T(t) };
                let vel = YT { y: Y(uniform(rng, [-0.1; K], [0.1; K])), t: T::default() };
                Particle::new(pos, vel)
            })
            .collect()
    }
}

// Level 3: sees the level-2 particle only.
#[derive(Clone, Debug)]
struct Fit3(Chain, YT);
impl OuterAware for Fit3 {
    type Outer = YT;
    fn see_outer(&mut self, yt: YT) {
        self.1 = yt;
    }
}
impl Objective for Fit3 {
    type Pos = T;
    fn score(&self, t: T) -> f64 {
        self.0.t_term(self.1.y.0, t.0)
    }
}

#[derive(Clone, Debug)]
struct TBounds(Chain, YT);
impl OuterAware for TBounds {
    type Outer = YT;
    fn see_outer(&mut self, yt: YT) {
        self.1 = yt;
    }
}
impl Region for TBounds {
    type Pos = T;
    fn confine(&self, t: T) -> T {
        let y = self.1.y.0;
        T(clamp(t.0, add(y, -self.0.w2), add(y, self.0.w2)))
    }
}

#[derive(Clone, Debug)]
struct Init3(Chain, usize);
impl Seeder for Init3 {
    type Outer = YT;
    type Inner = T;
    fn seed<R: Rng + ?Sized>(&self, yt: YT, rng: &mut R) -> Vec<Particle<T>> {
        let y = yt.y.0;
        (0..self.1)
            .map(|_| {
                let t = T(uniform(rng, add(y, -self.0.w2), add(y, self.0.w2)));
                Particle::new(t, T(uniform(rng, [-0.1; K], [0.1; K])))
            })
            .collect()
    }
}

struct Logs {
    level2: Log<W>,
    level3: Log<YT>,
}

fn level3(chain: Chain, seed: u64, log: &Log<YT>) -> impl Search<Pos = T> + OuterAware<Outer = YT> + Clone + Send {
    Swarm1::new(
        RecFit::new(Fit3(chain, YT::default()), log.clone()),
        Confined::new(
            PsoRule::new(CONFIG),
            RecBound::new(TBounds(chain, YT::default()), log.clone()),
        ),
    )
}

fn level2(
    chain: Chain,
    seed: u64,
    logs: &Logs,
    sizes: (usize, usize, usize, usize),
) -> impl Search<Pos = YT> + OuterAware<Outer = W> + Clone + Send {
    let (_, _, p3, n3) = sizes;
    Swarm2::new(
        RecFit::new(Fit2(chain, W::default()), logs.level2.clone()),
        Confined::new(
            PsoRule::new(CONFIG),
            RecBound::new(YBounds(chain, W::default()), logs.level2.clone()),
        )
        .for_field::<Y>(),
        InnerSearch::new(
            n3,
            level3(chain, seed + 1, &logs.level3),
            RecInit { inner: Init3(chain, p3), log: logs.level3.clone() }),
    )
}

fn solve(chain: Chain, seed: u64, logs: &Logs, sizes: (usize, usize, usize, usize), outer: (usize, usize)) -> W {
    let (p2, n2, _, _) = sizes;
    let (p1, n1) = outer;

    let mut searcher = Swarm2::new(
        Fit1(chain),
        Confined::new(PsoRule::new(CONFIG), XBox).for_field::<X>(),
        InnerSearch::new(
            n2,
            level2(chain, seed + 10, logs, sizes),
            RecInit { inner: Init2(chain, p2), log: logs.level2.clone() }),
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
fn each_level_sees_the_level_directly_above() {
    let chain = chain();
    let logs = Logs { level2: Log::new(), level3: Log::new() };
    let (p2, n2, p3, n3) = (3, 2, 2, 3);
    solve(chain, 7, &logs, (p2, n2, p3, n3), (3, 2));

    check_counts(&logs.level2.events(), p2 * n2, &["fit", "bound"]);
    check_counts(&logs.level3.events(), p3 * n3, &["fit", "bound"]);
}
