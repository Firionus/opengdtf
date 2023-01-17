use std::collections::HashMap;

use crate::types::{dmx_break::Break, name::Name};
use petgraph::graph::NodeIndex;

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
    Reference {
        reference: NodeIndex,
        offsets: Offsets,
    },
}

// TODO When Channel parsing is implemented, there needs to be a validation that
// each `Offsets` in a `GeometryReference` contains precisely the breaks
// required by channels operating on the referenced geometry. No more or less
// breaks are allowed to be present (see GDTF 1.2 page 39).
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Offsets {
    pub normal: HashMap<Break, u16>, // dmx_break => offset // TODO same validations as Offset
    pub overwrite: Option<Offset>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Offset {
    pub dmx_break: Break,
    pub offset: u16, // TODO more than 512 disallowed, 0 disallowed? negative disallowed?
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
