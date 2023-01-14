use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

use getset::Getters;
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::{graph::NodeIndex, Directed, Graph};

use crate::geometry_type::GeometryType;
use crate::types::name::Name;

use super::errors::GeometryError;

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
    /// Adds a top level geometry and returns its graph index.
    ///
    /// When the geometry name is already taken, does nothing and returns an Error.
    pub fn add_top_level(&mut self, geometry: GeometryType) -> Result<NodeIndex, GeometryError> {
        let new_name = geometry.name().to_owned();
        match self.names.entry(new_name) {
            Occupied(entry) => Err(GeometryError::NameAlreadyTaken(*entry.get())),
            Vacant(entry) => Ok(*entry.insert(self.graph.add_node(geometry))),
        }
    }

    /// Adds a geometry as the child of a parent and returns its graph index.
    ///
    /// When the `parent_graph_index` doesn't exist or the geometry name is
    /// already taken, does nothing and returns an Err.
    pub fn add(
        &mut self,
        geometry: GeometryType,
        parent_graph_index: NodeIndex,
    ) -> Result<NodeIndex, GeometryError> {
        let new_name = geometry.name().to_owned();
        if self.graph.node_weight(parent_graph_index).is_none() {
            return Err(GeometryError::MissingIndex(parent_graph_index));
        }
        match self.names.entry(new_name) {
            Occupied(entry) => Err(GeometryError::NameAlreadyTaken(*entry.get())),
            Vacant(entry) => {
                let new_ind = *entry.insert(self.graph.add_node(geometry));
                self.graph.add_edge(parent_graph_index, new_ind, ());
                Ok(new_ind)
            }
        }
    }

    // TODO ↓↓↓↓↓↓↓↓↓ continue code review below this point ↓↓↓↓↓↓↓↓↓↓

    /// Find the NodeIndex of a Geometry by its unique `Name`.
    pub fn find(&self, name: &Name) -> Option<NodeIndex> {
        // TODO better name, maybe get_index? (like get for Vec in std)
        // TODO return Option<&NodeIndex>?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_generation_and_access() {
        let mut g = Geometries::default();
        let a = g
            .add_top_level(GeometryType::Geometry {
                name: "a".try_into().unwrap(),
            })
            .unwrap();
        let b = g
            .add_top_level(GeometryType::Geometry {
                name: "b".try_into().unwrap(),
            })
            .unwrap();
        let a0 = g
            .add(
                GeometryType::Geometry {
                    name: "a0".try_into().unwrap(),
                },
                a,
            )
            .unwrap();
        let a0a = g
            .add(
                GeometryType::Geometry {
                    name: "a0a".try_into().unwrap(),
                },
                a0,
            )
            .unwrap();

        // adding same name again does not work
        assert!(matches!(
            g.add_top_level(GeometryType::Geometry {
                name: "a".try_into().unwrap(),
            }),
            Err(GeometryError::NameAlreadyTaken(i))
        if i == a));
        assert!(matches!(
            g.add(
                GeometryType::Geometry {
                    name: "a0a".try_into().unwrap(),
                },
                b
            ),
            Err(GeometryError::NameAlreadyTaken(i))
        if i == a0a));

        // adding with invalid parent index does not work
        assert!(matches!(
            g.add(
                GeometryType::Geometry {
                    name: "c".try_into().unwrap(),
                },
                NodeIndex::from(42)
            ),
            Err(GeometryError::MissingIndex(i))
        if i == 42.into()));

        // nodes can be found
        assert_eq!(g.find(&"a".try_into().unwrap()), Some(a));
        assert_eq!(g.find(&"b".try_into().unwrap()), Some(b));
        assert_eq!(g.find(&"a0".try_into().unwrap()), Some(a0));
        assert_eq!(g.find(&"a0a".try_into().unwrap()), Some(a0a));

        // nonexistent elements
        assert_eq!(g.find(&"c".try_into().unwrap()), None);
        assert_eq!(g.find(&"aa".try_into().unwrap()), None);
    }

    #[test]
    fn geometries_default_is_empty() {
        let geometries = Geometries::default();
        assert_eq!(geometries.graph().node_count(), 0);
        assert_eq!(geometries.names().len(), 0);
    }
}
