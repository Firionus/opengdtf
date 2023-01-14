use std::collections::HashMap;

use crate::types::name::Name;
use petgraph::graph::NodeIndex;

/// A geometry node in the geometry graph
#[derive(Debug, Clone)]
pub enum GeometryType {
    // TODO use composition for reuse of name, position, model
    Geometry {
        name: Name,
    },
    Reference {
        name: Name,
        reference: NodeIndex,
        offsets: Offsets,
    },
}

impl GeometryType {
    pub fn name(&self) -> &Name {
        match self {
            GeometryType::Geometry { name } | GeometryType::Reference { name, .. } => name,
        }
    }
}

// TODO When Channel parsing is implemented, there needs to be a validation that
// each `Offsets` in a `GeometryReference` contains the breaks required by
// channels operating on the referenced geometry. No more breaks are allowed to
// be serialized (see GDTF 1.2 page 39), but I think having them in the struct
// isn't bad (?).
#[derive(Debug, PartialEq, Clone)]
pub struct Offsets {
    pub normal: HashMap<u16, u16>, // dmx_break => offset // TODO same validations as Offset
    pub overwrite: Option<Offset>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Offset {
    pub dmx_break: u16, // TODO 0 disallowed?, is there an upper limit on breaks?
    pub offset: u16,    // TODO more than 512 disallowed, 0 disallowed? negative disallowed?
}

impl Offsets {
    pub fn new() -> Self {
        Offsets {
            normal: HashMap::new(),
            overwrite: None,
        }
    }
}

impl Default for Offsets {
    fn default() -> Self {
        Self::new()
    }
}
