use std::{io, path::Path};

use thiserror::Error;
use zip::result::ZipError;

/// An unrecoverable GDTF Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid XML: {0}")]
    XmlError(#[from] roxmltree::Error),
    #[error("root node 'GDTF' not found")]
    NoRootNode,
    #[error("could not open file '{0}': {1}")]
    OpenError(Box<Path>, io::Error),
    #[error("zip error: {0}")]
    ZipError(#[from] ZipError),
    #[error("'description.xml' not found in GDTF zip archive: {0}")]
    DescriptionXmlMissing(ZipError),
    #[error("'description.xml' could not be read: {0}")]
    DescriptionXmlReadError(io::Error),
}

/// A Problem in a GDTF file that is recoverable with a sensible empty or default value.
#[derive(Error, Debug, PartialEq)]
pub enum Problem {
    #[error("missing attribute 'DataVersion' on 'GDTF' node")]
    NoDataVersion,
    #[error("attribute 'DataVersion' of 'GDTF' node is invalid. Got '{0}'.")]
    InvalidDataVersion(String),
    #[error("node '{missing}' missing as child of '{parent}'")]
    NodeMissing { missing: String, parent: String },
    #[error("attribute '{attr}' missing on node '{node}'")]
    AttributeMissing { attr: String, node: String },
    #[error("UUID error in {1}: {0}")]
    UuidError(uuid::Error, String),
    #[error("invalid enum string in {1}. Expected one of ['Yes', 'No']. Got {0}")]
    InvalidYesNoEnum(String, String),
    #[error("error with Geometry tree: {0}")]
    GeometryTreeError(String),
}