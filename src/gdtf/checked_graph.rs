use std::fmt::Debug;

use delegate::delegate;
use petgraph::{
    graph::DefaultIx, graph::EdgeIndex, graph::IndexType, graph::NodeIndex, Directed, EdgeType,
    Graph,
};

/// A newtype around petgraph's Graph that tries to not panic with invalid arguments
/// and instead returns a Result.
#[derive(derive_more::Display, derive_more::DebugCustom, Clone)]
#[debug(fmt = "{_0:?}")]
#[debug(bound = "N: Debug, E: Debug")]
pub struct CheckedGraph<N, E, Ty = Directed, Ix = DefaultIx>(Graph<N, E, Ty, Ix>)
where
    Ty: EdgeType,
    Ix: IndexType;

// only needed because N and E don't have to support Default for this to work
// TODO replace either by derivative or ambassador
// see https://mcarton.github.io/rust-derivative/latest/Default.html#custom-bound
// or https://github.com/hobofan/ambassador#delegate-target--foo---target-key
impl<N, E, Ty, Ix> Default for CheckedGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn default() -> Self {
        Self(Graph::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CheckedGraphError {
    #[error("graph is at the maximum number of nodes")]
    MaximumNodesReached,
    #[error("graph is at the maximum number of edges")]
    MaximumEdgesReached,
    #[error("invalid node index")]
    InvalidNodeIndex,
}

impl<N, E, Ty, Ix> CheckedGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    pub fn add_node(&mut self, weight: N) -> Result<NodeIndex<Ix>, CheckedGraphError> {
        let new_idx: NodeIndex<Ix> = NodeIndex::new(self.0.node_count());
        if self.0.node_count() <= (isize::MAX as usize) && new_idx < NodeIndex::end() {
            Ok(self.0.add_node(weight))
        } else {
            Err(CheckedGraphError::MaximumNodesReached)
        }
    }

    pub fn add_edge(
        &mut self,
        a: NodeIndex<Ix>,
        b: NodeIndex<Ix>,
        weight: E,
    ) -> Result<EdgeIndex<Ix>, CheckedGraphError> {
        let new_idx: NodeIndex<Ix> = NodeIndex::new(self.0.edge_count());
        if self.0.edge_count() <= (isize::MAX as usize) && new_idx < NodeIndex::end() {
            if self.0.node_weight(a).is_some() && self.0.node_weight(b).is_some() {
                Ok(self.0.add_edge(a, b, weight))
            } else {
                Err(CheckedGraphError::InvalidNodeIndex)
            }
        } else {
            Err(CheckedGraphError::MaximumEdgesReached)
        }
    }

    // non-panicking methods can be delegated
    delegate! {
        to self.0 {
            pub fn node_weight(&self, a: NodeIndex<Ix>) -> Option<&N>;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_transparent() {
        let mut graph = Graph::<i32, i32>::new();
        let a = graph.add_node(42);
        let b = graph.add_node(27);
        graph.add_edge(a, b, 13);
        let graph_debug = format!("{graph:?}");
        let checked = CheckedGraph(graph);
        let checked_debug = format!("{checked:?}");
        assert_eq!(graph_debug, checked_debug);
    }

    #[test]
    fn add_edge_does_not_panic() -> Result<(), CheckedGraphError> {
        let mut g = CheckedGraph::<i32, i32>::default();
        let a = g.add_node(42)?;
        let b = g.add_node(17)?;
        g.add_edge(a, b, 13)?;
        assert!(matches!(
            g.add_edge(NodeIndex::new(100), b, 0),
            Err(CheckedGraphError::InvalidNodeIndex)
        ));
        Ok(())
    }
}
