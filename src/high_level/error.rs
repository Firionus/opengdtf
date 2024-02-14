use std::num::NonZeroU8;

use crate::Name;

/// Errors in high level GDTF.
#[derive(thiserror::Error, Debug)]
pub enum GdtfError {
    #[error("geometry name '{0}' already exists but must be unique")]
    DuplicateGeometryName(Name),
    #[error("geometry name '{0}' not found")]
    UnknownGeometryName(Name),
    #[error("top level geometry name '{0}' not found")]
    UnknownTopLevelGeometryName(Name),
    #[error(
        "top-level geometry references are not allowed to avoid superfluous \
        reference chains and because you can just offset associated DMX channels manually"
    )]
    TopLevelGeometryReference(),
    #[error("geometry '{0}' can't have children")]
    ChildFreeGeometryType(Name),
    #[error("default break '{0}' does not have an associated offset")]
    InvalidDefaulBreak(NonZeroU8),
    #[error(
        "unexpected condition occured. This is a fault in opengdtf. \
        Please open an issue at https://github.com/Firionus/opengdtf/issues/new. Caused by: {0}"
    )]
    Unexpected(Box<dyn std::error::Error + Send + Sync>),
}
