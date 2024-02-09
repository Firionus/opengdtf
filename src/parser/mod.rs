#![allow(clippy::result_large_err)]
// TODO fix warning later, it is only a memory usage problem, due to an enum
// variant in `ProblemType` with many fields
pub mod errors;
pub mod parse;
mod parse_xml;
pub mod problems;
pub mod validate;

pub use self::{
    errors::Error,
    parse::parse,
    problems::{HandledProblem, Problem, ProblemAt, Problems},
    validate::ValidatedGdtf,
};
