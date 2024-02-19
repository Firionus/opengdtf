#![allow(clippy::result_large_err)]
// TODO fix warning later, it is only a memory usage problem, due to an enum
// variant in `ProblemType` with many fields

pub(crate) mod parse_xml;

pub mod low_level;

mod error;
mod high_level;
mod problems;

pub use self::{error::*, high_level::*, problems::*};
