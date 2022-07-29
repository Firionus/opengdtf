use std::{io, path::Path};

use thiserror::Error;
use zip::result::ZipError;

#[derive(Error, Debug)]
pub enum GdtfCompleteFailure {
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

#[derive(Error, Debug, PartialEq)]
pub enum GdtfProblem {
    #[error("missing attribute 'DataVersion' on 'GDTF' node")]
    NoDataVersion,
    #[error("attribute 'DataVersion' of 'GDTF' node is invalid. Got '{0}'.")]
    InvalidDataVersion(String),
}