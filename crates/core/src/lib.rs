pub mod alignment;
mod diagnostics;
mod model;
pub mod normalization;
mod validation;

pub use alignment::*;
pub use diagnostics::*;
pub use model::*;
pub use normalization::*;
pub use validation::*;

pub const SCHEMA_VERSION: u16 = 1;
