//! Reproducible random streams. Every consumer of randomness gets a stream
//! derived from the search's seed and its own coordinates (iteration,
//! component slot, particle), so no consumer's draws affect another's, and
//! nothing depends on which thread does the work.

use rand::SeedableRng;
use rand::rngs::StdRng;

const RUN_TAG: u64 = 0x5255_4e5f_5345_4544;
const RULE_TAG: u64 = 0x5255_4c45_5f53_5452;
const SEEDER_TAG: u64 = 0x5345_4544_4552_5f31;

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn mix(a: u64, b: u64) -> u64 {
    splitmix(a ^ splitmix(b ^ 0xD6E8_FEB8_6659_FD93))
}

/// The seed of a top-level search.
pub(crate) fn run_seed(seed: u64) -> u64 {
    mix(seed, RUN_TAG)
}

/// The stream of one rule, for one particle, in one iteration.
pub(crate) fn rule_stream(seed: u64, iteration: u64, slot: usize, particle: usize) -> StdRng {
    let s = mix(mix(mix(mix(seed, RULE_TAG), iteration), slot as u64), particle as u64);
    StdRng::seed_from_u64(s)
}

/// The stream a seeder gets for one search.
pub(crate) fn seeder_stream(seed: u64) -> StdRng {
    StdRng::seed_from_u64(mix(seed, SEEDER_TAG))
}
