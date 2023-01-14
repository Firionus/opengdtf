use petgraph::graph::NodeIndex;
/// domain errors go here
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeometryError {
    #[error("geometry name already taken by geometry with index {0:?}")]
    NameAlreadyTaken(NodeIndex),
    #[error("missing geometry graph index {0:?}")]
    MissingIndex(NodeIndex),
}
