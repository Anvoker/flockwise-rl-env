//! A real user of the library: planning drone delivery routes.

pub mod model;
mod plan;

pub use plan::{Budget, plan};
