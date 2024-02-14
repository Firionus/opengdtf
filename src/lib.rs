#![warn(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

// these are not re-exported as they are somehwat niche.
// If the user needs them, they have to be qualified
pub mod hash;
pub mod low_level;

// these modules are re-exported as they form the main part of the API
mod high_level;
pub mod parse;
pub mod serialize;

pub use high_level::*;
pub use parse::*;
pub use serialize::*;
