//! Point types and problems shared by the tests. Written by users of the
//! library: only the library's public names are used.
#![allow(dead_code)]

use flockwise::{Particle, PsoParams};
use rand::Rng;

/// Defines a fixed-size vector unit: `vector_unit!(X, 3);`
#[macro_export]
macro_rules! vector_unit {
    ($name:ident, $k:expr) => {
        #[derive(Copy, Clone, Debug, Default, PartialEq)]
        pub struct $name(pub [f64; $k]);

        impl std::ops::Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                let mut out = self.0;
                for i in 0..$k {
                    out[i] += rhs.0[i];
                }
                $name(out)
            }
        }

        impl std::ops::Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                let mut out = self.0;
                for i in 0..$k {
                    out[i] -= rhs.0[i];
                }
                $name(out)
            }
        }

        impl std::ops::Mul<f64> for $name {
            type Output = Self;
            fn mul(self, rhs: f64) -> Self {
                let mut out = self.0;
                for v in out.iter_mut() {
                    *v *= rhs;
                }
                $name(out)
            }
        }

        impl flockwise::Point for $name {
            fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
                $name(self.0.map(|v| f(v)))
            }
        }
    };
}

/// Defines a unit made of two parts: `pair_unit!(W, x: X, t: T);`
#[macro_export]
macro_rules! pair_unit {
    ($name:ident, $a:ident : $ta:ty, $b:ident : $tb:ty) => {
        #[derive(Copy, Clone, Debug, Default, PartialEq)]
        pub struct $name {
            pub $a: $ta,
            pub $b: $tb,
        }

        impl std::ops::Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                $name { $a: self.$a + rhs.$a, $b: self.$b + rhs.$b }
            }
        }

        impl std::ops::Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                $name { $a: self.$a - rhs.$a, $b: self.$b - rhs.$b }
            }
        }

        impl std::ops::Mul<f64> for $name {
            type Output = Self;
            fn mul(self, rhs: f64) -> Self {
                $name { $a: self.$a * rhs, $b: self.$b * rhs }
            }
        }

        impl flockwise::Point for $name {
            fn map_scalars(self, mut f: impl FnMut(f64) -> f64) -> Self {
                $name { $a: self.$a.map_scalars(&mut f), $b: self.$b.map_scalars(&mut f) }
            }
        }

        $crate::field_of!($name, $a: $ta);
        $crate::field_of!($name, $b: $tb);
    };
}

/// Declares that field `$f` of type `$ty` is part of `$whole`.
#[macro_export]
macro_rules! field_of {
    ($whole:ident, $f:ident : $ty:ty) => {
        impl flockwise::SplitFrom for $ty {
            type Whole = $whole;
            fn split(w: flockwise::ParticleView<'_, $whole>) -> flockwise::ParticleView<'_, Self> {
                flockwise::ParticleView {
                    pos: &mut w.pos.$f,
                    vel: &mut w.vel.$f,
                    score: w.score,
                    best_pos: &mut w.best_pos.$f,
                    best_score: w.best_score,
                }
            }
        }

        impl flockwise::ReadFrom for $ty {
            type Whole = $whole;
            fn read(whole: &$whole) -> Self {
                whole.$f
            }
        }

        impl flockwise::WriteInto for $ty {
            type Whole = $whole;
            fn write(self, whole: &mut $whole) {
                whole.$f = self;
            }
        }
    };
}

pub const CONFIG: PsoParams = PsoParams {
    inertia: 0.7,
    cognitive: 1.5,
    social: 1.5,
};

pub fn uniform<R: Rng + ?Sized, const K: usize>(rng: &mut R, lo: [f64; K], hi: [f64; K]) -> [f64; K] {
    let mut out = [0.0; K];
    for i in 0..K {
        out[i] = rng.random_range(lo[i]..=hi[i]);
    }
    out
}

pub fn clamp<const K: usize>(v: [f64; K], lo: [f64; K], hi: [f64; K]) -> [f64; K] {
    let mut out = v;
    for i in 0..K {
        out[i] = v[i].clamp(lo[i], hi[i]);
    }
    out
}

pub fn add<const K: usize>(v: [f64; K], d: f64) -> [f64; K] {
    v.map(|x| x + d)
}

pub fn max_abs_diff<const K: usize>(a: [f64; K], b: [f64; K]) -> f64 {
    (0..K).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max)
}

/// Squared distance. Uses `d * d`, which IEEE 754 requires to be exact, and
/// not `powi`, whose precision Rust doesn't guarantee (Miri varies it).
pub fn sq_dist<const K: usize>(a: [f64; K], b: [f64; K]) -> f64 {
    (0..K).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

pub fn particles<T: Copy>(positions: Vec<(T, T)>) -> Vec<Particle<T>> {
    positions.into_iter().map(|(pos, vel)| Particle::new(pos, vel)).collect()
}

pub mod two_part;

/// Offset added to every seed, so the numeric tests can be run over many
/// seeds: `FLOCKWISE_SEED_BASE=1000 cargo test`.
pub fn seed_base() -> u64 {
    std::env::var("FLOCKWISE_SEED_BASE").ok().and_then(|s| s.parse().ok()).unwrap_or(0)
}
