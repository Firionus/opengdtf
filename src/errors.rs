use thiserror::Error;

#[derive(Error, Debug)]
pub enum GdtfCompleteFailure {
    #[error("invalid XML: {0}")]
    XmlError(#[from] roxmltree::Error),
    #[error("root node 'GDTF' not found")]
    NoRootNode,
}

#[derive(Error, Debug, PartialEq)]
pub enum GdtfProblem {
    #[error("missing attribute 'DataVersion' on 'GDTF' node")]
    NoDataVersion,
    #[error("attribute 'DataVersion' of 'GDTF' node is invalid. Got '{0}'.")]
    InvalidDataVersion(String),
}