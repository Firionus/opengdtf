use std::{io, path::Path};

use roxmltree::{Document, TextPos};
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

// TODO these problems could report positions in the input files, like shown here:
// https://github.com/RazrFalcon/roxmltree/blob/master/examples/print_pos.rs

/// A Problem in a GDTF file that is recoverable with a sensible empty or default value.
#[derive(Error, Debug, PartialEq)]
pub enum Problem {
    #[error("missing attribute 'DataVersion' on 'GDTF' node")] // TODO add pos
    NoDataVersion,
    #[error("attribute 'DataVersion' of 'GDTF' node is invalid. Got '{0}'.")] // TODO add pos
    InvalidDataVersion(String),
    #[error("node '{missing}' missing as child of '{parent}'")] // TODO add pos
    XmlNodeMissing { missing: String, parent: String },
    #[error("attribute '{attr}' missing on '{tag}' node at {pos}")]
    XmlAttributeMissing { attr: String, tag: String, pos: TextPos },
    #[error("UUID error in {1}: {0}")] // TODO add pos
    UuidError(uuid::Error, String),
    #[error("invalid enum string in {1}. Expected one of ['Yes', 'No']. Got {0}")] // TODO add pos
    InvalidYesNoEnum(String, String),
    #[error("error with Geometry tree: {0}")] // TODO remove?
    GeometryTreeError(String),
}

pub fn node_position(node: &roxmltree::Node, doc: &Document) -> TextPos {
    doc.text_pos_at(node.range().start)
}