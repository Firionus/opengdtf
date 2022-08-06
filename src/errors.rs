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

// TODO Do all of these Problems indicate what action was taken? Like "Renamed to
// 'DuplicateGeometry <UUID>'"? Can we actually say that? In a lot of cases, we
// might have different actions depending on the context, e.g. with
// XmlAttributeMissing, sometimes we abort parsing an element, sometimes we
// insert a default, sometimes we indicate missing, ...

/// A Problem in a GDTF file that is recoverable with a sensible empty or default value.
#[derive(Error, Debug)]
pub enum Problem {
    #[error("missing attribute 'DataVersion' on 'GDTF' node at line {0}")]
    NoDataVersion(TextPos),
    #[error("attribute 'DataVersion' of 'GDTF' node at line {1} is invalid. Got '{0}'.")]
    InvalidDataVersion(String, TextPos),
    #[error("node '{missing}' missing as child of '{parent}' at line {pos}")]
    XmlNodeMissing {
        missing: String,
        parent: String,
        pos: TextPos,
    },
    #[error("attribute '{attr}' missing on '{tag}' node at line {pos}")]
    XmlAttributeMissing {
        attr: String,
        tag: String,
        pos: TextPos,
    },
    #[error("attribute '{attr}' on '{tag}' at line {pos} could not be parsed as {expected_type}. Value '{content}' caused error: {err}")]
    InvalidAttribute {
        attr: String,
        tag: String,
        pos: TextPos,
        content: String,
        err: Box<dyn std::error::Error>,
        expected_type: String,
    },
    #[error("unexpected XML node '{0}' at line {1}")]
    UnexpectedXmlNode(String, TextPos),
    #[error("UUID error in '{1}' at line {2}: {0}")]
    UuidError(uuid::Error, String, TextPos),
    #[error("invalid enum string in {1} at line {2}. Expected one of ['Yes', 'No']. Got {0}")]
    InvalidYesNoEnum(String, String, TextPos),
    #[error("duplicate Geometry name '{0}' at line {1}")]
    DuplicateGeometryName(String, TextPos),
    #[error("duplicate DMXBreak attribute '{0}' at line {1}")]
    DuplicateDmxBreak(u16, TextPos),
    #[error("unexpected 'GeometryReference' as top-level Geometry at line {0}")]
    UnexpectedTopLevelGeometryReference(TextPos),
    #[error("Geometry '{0}' is referenced at line {1} but not found")]
    UnknownGeometry(String, TextPos),
    #[error(
        "Geometry '{0}' is referenced at line {1} and was expected to be top-level but wasn't"
    )]
    NonTopLevelGeometryReferenced(String, TextPos),
}

pub fn node_position(node: &roxmltree::Node, doc: &Document) -> TextPos {
    doc.text_pos_at(node.range().start)
}

pub(crate) trait ProblemAdd {
    /// Push a Problem and Return None
    fn push_then_none<T>(&mut self, e: Problem) -> Option<T>;
}

impl ProblemAdd for Vec<Problem> {
    fn push_then_none<T>(&mut self, e: Problem) -> Option<T> {
        self.push(e);
        None
    }
}