//! flockwise: composable, nested, parallel and reproducible particle swarm
//! searches.

mod api;
mod common;
mod rng;
mod rules;
pub mod routes;
mod swarm;

pub use api::*;
pub use common::*;
pub use rules::*;
pub use swarm::*;
