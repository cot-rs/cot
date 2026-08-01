#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
// darling 0.24's derive codegen emits redundant path qualifications that
// `unused_qualifications` flags at the field spans; the lint can't be silenced
// at the item level, so it is allowed crate-wide.
#![allow(unused_qualifications)]

pub mod expr;
pub mod model;
pub mod symbol_resolver;
