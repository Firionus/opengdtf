use std::collections::HashMap;

use getset::Getters;
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::{graph::NodeIndex, Directed, Graph};

use crate::types::name::Name;

#[derive(Debug, Default, Getters)]
#[getset(get = "pub")]
pub struct Geometries {
    /// Maps geometry name to its graph index. Use for quick name lookup.
    names: HashMap<Name, NodeIndex>,

    /// Graph representing the geometry tree. Edges point from parent to child.
    ///
    /// Petgraph is used to avoid having to learn multiple graph/tree-libraries
    /// for this crate. The tree structure is ensured by the modifying methods
    /// and the fact that the field is not mutably accesible from the outside.
    graph: GeometryGraph,
}

type GeometryGraph = Graph<GeometryType, (), Directed>;

impl Geometries {
    // TODO replace with `add` and `add_top_level`, then there is no Option in
    // the Arguments, which is somewhat confusing :)

    /// Adds a Geometry and returns the NodeIndex of the new geometry
    ///
    /// If you want to add a top-level geometry, set parent_index to `None`. If
    /// a geometry of the same name is already present, does not do anything and
    /// returns `None`.
    pub fn add(
        &mut self,
        geometry: GeometryType,
        parent_index: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        let new_name = geometry.name().to_owned();

        if self.names.contains_key(&new_name) {
            return None;
        }

        let new_ind = self.graph.add_node(geometry);
        if let Some(parent_index) = parent_index {
            self.graph.add_edge(parent_index, new_ind, ());
        };
        self.names.insert(new_name, new_ind);
        Some(new_ind)
    }

    /// Find the NodeIndex of a Geometry by its unique `Name`.
    pub fn find(&self, name: &Name) -> Option<NodeIndex> {
        self.names.get(name).map(|i| i.to_owned())
    }

    /// Checks if the Geometry with given `NodeIndex` `i` is a top-level geometry.
    ///
    /// If geometry with index `i` doesn't exist, `true` is returned.
    pub fn is_top_level(&self, i: NodeIndex) -> bool {
        self.graph.edges_directed(i, Incoming).next().is_none()
    }

    /// Returns the number of children of the Geometry with index `i`.
    ///
    /// If geometry `i` does not exist, zero is returned.
    pub fn count_children(&self, i: NodeIndex) -> usize {
        self.graph.edges_directed(i, Outgoing).count()
    }

    pub fn children(&self, i: NodeIndex) -> impl Iterator<Item = &GeometryType> {
        self.graph.neighbors(i).map(|ind| &self.graph[ind])
    }

    pub fn qualified_name(&self, ind: NodeIndex) -> String {
        // TODO indexing may panic - what to do?
        let n = &self.graph[ind];
        let mut qualified_name = n.name().to_string();
        let mut i = ind;
        while let Some(parent_ind) = self.graph.neighbors_directed(i, Incoming).next() {
            // indexing won't panic because ind comes from graph iterator
            qualified_name = format!("{}.{}", self.graph[parent_ind].name(), qualified_name);
            // TODO prepending like this probably isn't very performant ;)
            i = parent_ind
        }
        qualified_name
    }

    pub fn parent_ind(&self, ind: NodeIndex) -> Option<NodeIndex> {
        self.graph.neighbors_directed(ind, Incoming).next()
    }

    pub fn top_level_geometry(&self, ind: NodeIndex) -> NodeIndex {
        let mut i = ind;
        while let Some(parent_ind) = self.parent_ind(i) {
            i = parent_ind
        }
        i
    }
}

// TODO When Channel parsing is implemented, there needs to be a validation that
// each `Offsets` in a `GeometryReference` contains the breaks required by
// channels operating on the referenced geometry. No more breaks are allowed to
// be serialized (see GDTF 1.2 page 39), but I think having them in the struct
// isn't bad.
#[derive(Debug, PartialEq, Clone)]
pub struct Offsets {
    pub normal: HashMap<u16, u16>, // dmx_break => offset // TODO same validations as Offset
    pub overwrite: Option<Offset>,
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

#[derive(Debug, PartialEq, Clone)]
pub struct Offset {
    pub dmx_break: u16, // TODO 0 disallowed?, is there an upper limit on breaks?
    pub offset: u16,    // TODO more than 512 disallowed, 0 disallowed? negative disallowed?
}

// TODO use composition for reuse of name, position, model
#[derive(Debug, Clone)]
pub enum GeometryType {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let mut g = Geometries::default();
        let top_1 = g
            .add(
                GeometryType::Geometry {
                    name: "top 0".try_into().unwrap(),
                },
                None,
            )
            .unwrap();
        let _child_0 = g
            .add(
                GeometryType::Geometry {
                    name: "child 0".try_into().unwrap(),
                },
                Some(top_1),
            )
            .unwrap();
        // adding same name again does not work
        assert_eq!(
            g.add(
                GeometryType::Geometry {
                    name: "top 0".try_into().unwrap(),
                },
                Some(top_1),
            ),
            None
        )
    }
}
