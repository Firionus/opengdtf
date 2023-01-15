use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

use getset::Getters;
use petgraph::visit::Walker;
use petgraph::Direction::Incoming;
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
        let parent_graph_index = self.validate_index(parent_graph_index)?;
        match self.names.entry(new_name) {
            Occupied(entry) => Err(GeometryError::NameAlreadyTaken(*entry.get())),
            Vacant(entry) => {
                let new_ind = *entry.insert(self.graph.add_node(geometry));
                self.graph.add_edge(parent_graph_index, new_ind, ());
                Ok(new_ind)
            }
        }
    }

    /// Get the graph index of a Geometry by its unique `Name`
    pub fn get_index(&self, name: &Name) -> Option<NodeIndex> {
        self.names
            .get(name)
            .map(|graph_index| graph_index.to_owned())
    }

    /// Wraps the graph index in Ok if a geometry with this graph index exists
    pub fn validate_index(&self, graph_index: NodeIndex) -> Result<NodeIndex, GeometryError> {
        if self.graph.node_weight(graph_index).is_none() {
            Err(GeometryError::MissingIndex(graph_index))
        } else {
            Ok(graph_index)
        }
    }

    /// Returns the graph index of the parent of the geometry with the given
    /// graph_index, or None if the geometry is top level or missing.
    pub fn parent_index(&self, graph_index: NodeIndex) -> Option<NodeIndex> {
        self.graph.neighbors_directed(graph_index, Incoming).next()
    }

    /// Checks if the Geometry with given graph index is a top-level geometry.
    ///
    /// If no geometry with the graph index exists, true is returned.
    pub fn is_top_level(&self, graph_index: NodeIndex) -> bool {
        self.parent_index(graph_index).is_none()
    }

    /// Returns the number of children of the geometry with given graph index.
    ///
    /// If no geometry with the graph index exists, zero is returned.
    pub fn count_children(&self, graph_index: NodeIndex) -> usize {
        self.graph.neighbors(graph_index).count()
    }

    /// Returns an iterator over the children of geometry with given graph index
    ///
    /// If no geometry with given graph index exists, an empty iterator is returned.
    pub fn children_geometries(
        &self,
        graph_index: NodeIndex,
    ) -> impl Iterator<Item = &GeometryType> {
        self.graph.neighbors(graph_index).map(|i| &self.graph[i])
    }

    /// Returns an iterator over the indices of all the ancestors of the
    /// geometry with given graph index, all the way up to its
    /// top level geometry.
    pub fn ancestors(&self, graph_index: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        GeometryAncestors { i: graph_index }.iter(self)
    }

    /// Returns the fully qualified name of the geometry with given graph index.
    ///
    /// If no geometry with given graph index exists, an empty String is returned.
    pub fn qualified_name(&self, graph_index: NodeIndex) -> String {
        let n = match self.graph.node_weight(graph_index) {
            Some(n) => n,
            None => return "".to_string(),
        };
        let mut qualified_name = n.name().to_string();
        self.ancestors(graph_index).for_each(|parent_index| {
            qualified_name = format!("{}.{}", self.graph[parent_index].name(), qualified_name)
            // TODO prepending like this probably isn't particularly performant
        });
        qualified_name
    }

    /// Returns the graph index of the geometry with given graph index.
    ///
    /// If graph_index doesn't exist or the geometry is top level itself, the
    /// input index is returned.
    pub fn top_level_geometry_index(&self, graph_index: NodeIndex) -> NodeIndex {
        self.ancestors(graph_index).last().unwrap_or(graph_index)
    }
}

pub struct GeometryAncestors {
    i: NodeIndex,
}

impl Walker<&Geometries> for GeometryAncestors {
    type Item = NodeIndex;

    fn walk_next(&mut self, context: &Geometries) -> Option<Self::Item> {
        self.i = context.parent_index(self.i)?;
        Some(self.i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_generation_and_access() {
        let mut g = Geometries::default();
        let nonexistent_graph_index = NodeIndex::new(42);

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

        assert!(matches!(
            g.add(
                GeometryType::Geometry {
                    name: "c".try_into().unwrap(),
                },
                nonexistent_graph_index
            ),
            Err(GeometryError::MissingIndex(i))
        if i == nonexistent_graph_index));

        assert_eq!(g.get_index(&"a".try_into().unwrap()), Some(a));
        assert_eq!(g.get_index(&"b".try_into().unwrap()), Some(b));
        assert_eq!(g.get_index(&"a0".try_into().unwrap()), Some(a0));
        assert_eq!(g.get_index(&"a0a".try_into().unwrap()), Some(a0a));
        assert_eq!(g.get_index(&"c".try_into().unwrap()), None);
        assert_eq!(g.get_index(&"aa".try_into().unwrap()), None);

        assert!(g.is_top_level(a));
        assert!(g.is_top_level(b));
        assert!(!g.is_top_level(a0));
        assert!(!g.is_top_level(a0a));
        assert!(g.is_top_level(nonexistent_graph_index));

        assert_eq!(g.count_children(a), 1);
        assert_eq!(g.count_children(b), 0);
        assert_eq!(g.count_children(a0), 1);
        assert_eq!(g.count_children(a0a), 0);
        assert_eq!(g.count_children(nonexistent_graph_index), 0);

        assert_eq!(g.parent_index(a), None);
        assert_eq!(g.parent_index(a0), Some(a));
        assert_eq!(g.parent_index(a0a), Some(a0));
        assert_eq!(g.parent_index(nonexistent_graph_index), None);

        assert_eq!(g.top_level_geometry_index(a), a);
        assert_eq!(g.top_level_geometry_index(a0), a);
        assert_eq!(g.top_level_geometry_index(a0a), a);
        assert_eq!(
            g.top_level_geometry_index(nonexistent_graph_index),
            nonexistent_graph_index
        );

        let mut a0a_ancestors = g.ancestors(a0a);
        assert_eq!(a0a_ancestors.next(), Some(a0));
        assert_eq!(a0a_ancestors.next(), Some(a));
        assert_eq!(a0a_ancestors.next(), None);

        assert_eq!(g.qualified_name(a0a), "a.a0.a0a");
        assert_eq!(g.qualified_name(a0), "a.a0");
        assert_eq!(g.qualified_name(a), "a");
    }

    #[test]
    fn geometries_default_is_empty() {
        let geometries = Geometries::default();
        assert_eq!(geometries.graph().node_count(), 0);
        assert_eq!(geometries.names().len(), 0);
    }
}
