use thiserror::Error;

#[derive(Error, Debug)]
pub enum GdtfCompleteFailure {
    #[error("invalid XML: {0}")]
    XmlError(#[from] roxmltree::Error),
    #[error("root node 'GDTF' not found")]
    NoRootNode,
}