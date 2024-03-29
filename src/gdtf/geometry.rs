use std::collections::HashMap;

use crate::{dmx_break::Break, name::Name};

/// A geometry node in the geometry graph
#[derive(Debug, Clone)]
pub struct Geometry {
    pub name: Name,
    pub t: Type,
}

/// The Geometry Type as indicated by the XML tag name
#[derive(Debug, Clone)]
pub enum Type {
    General,
    Reference { offsets: Offsets }, // referenced top level geometry kept in `templates` graph
}

// TODO When Channel parsing is implemented, there needs to be a validation that
// each `Offsets` in a `GeometryReference` contains precisely the breaks
// required by channels operating on the referenced geometry. No more or less
// breaks are allowed to be present (see GDTF 1.2 page 39).
// This might require a new data structure to hold the offsets for all
// geometry references for a certain abstract top level geometry.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Offsets {
    pub normal: HashMap<Break, i32>, // TODO currently 1-based. 0-based would be easier internally...
    pub overwrite: Option<Offset>, // TODO make this mandatory, if it's not there it means there are no offsets at all and we might as well give up the whole GeometryReference...
}

#[derive(Debug, PartialEq, Clone)]
pub struct Offset {
    pub dmx_break: Break,
    /// should support "Universe.Address" format according to standard, but that
    /// is not implemented at the moment
    pub offset: i32, // TODO is there validation on it?
                     // TODO GDTF Builder disallows anything less than 1, so maybe we should switch this to validated 1..512 u16
                     // TODO consider https://docs.rs/bounded-integer/latest/bounded_integer/
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_default_is_empty() {
        let offsets = Offsets::default();
        assert_eq!(offsets.normal.len(), 0);
        assert_eq!(offsets.overwrite, None);
    }
}
