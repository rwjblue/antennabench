pub mod alignment;
mod diagnostics;
mod model;
pub mod normalization;
mod operator_events;
mod semantics;
mod v2;
mod v3;
mod validation;

pub use alignment::*;
pub use diagnostics::*;
pub use model::*;
pub use normalization::*;
pub use operator_events::*;
pub use semantics::*;
pub use v2::*;
pub use v3::*;
pub use validation::*;

/// Schema used by the legacy adapter APIs. New durable bundles use v2.
pub const SCHEMA_VERSION: u16 = SCHEMA_VERSION_V1;
pub const SCHEMA_VERSION_V1: u16 = 1;
pub const SCHEMA_VERSION_V2: u16 = 2;
/// Selected by ADR 0016; storage dispatch remains v2 until #86 lands fully.
pub const SCHEMA_VERSION_V3: u16 = 3;
pub const LATEST_SCHEMA_VERSION: u16 = SCHEMA_VERSION_V2;
