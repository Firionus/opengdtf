use std::io;

use roxmltree::TextPos;
use thiserror::Error;
use zip::result::ZipError;

use crate::Problems;

use super::utils::XmlPosition;

/// An unrecoverable GDTF Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid XML: {0}")]
    InvalidXml(#[from] roxmltree::Error),
    #[error("root node 'GDTF' not found")]
    NoRootNode,
    #[error("zip error: {0}")]
    InvalidZip(#[from] ZipError),
    #[error("'description.xml' not found in GDTF zip archive: {0}")]
    DescriptionXmlMissing(ZipError),
    #[error("'description.xml' could not be read: {0}")]
    InvalidDescriptionXml(io::Error),
    #[error(
        "unexpected condition occured. This is a fault in opengdtf. \
    Please open an issue at https://github.com/Firionus/opengdtf/issues/new. Caused by: {0}"
    )]
    Unexpected(String),
}

#[derive(Error, Debug)]
#[error("{p} (line {at})")]
pub struct Problem {
    p: ProblemType,
    at: TextPos,
}

impl Problem {
    pub fn handled_by<T: Into<String>>(self, action: T, problems: &mut Problems) {
        problems.push(HandledProblem {
            p: self,
            action: action.into(),
        });
    }
}

pub(crate) trait HandleProblem<T, S: Into<String>> {
    fn handled_by(self, action: S, problems: &mut Problems) -> Option<T>;
}

impl<T, S: Into<String>> HandleProblem<T, S> for Result<T, Problem> {
    fn handled_by(self, action: S, problems: &mut Problems) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(p) => {
                p.handled_by(action, problems);
                None
            }
        }
    }
}

#[derive(Error, Debug)]
#[error("{p}; {action}")]
pub struct HandledProblem {
    p: Problem,
    pub action: String,
}

impl HandledProblem {
    pub fn problem_type(&self) -> &ProblemType {
        &self.p.p
    }
}

/// A Problem in a GDTF file that is recoverable with a sensible empty or default value.
#[derive(Error, Debug)]
pub enum ProblemType {
    #[error("missing node '{missing}' as child of '{parent}'")]
    XmlNodeMissing { missing: String, parent: String },
    #[error("missing attribute '{attr}' on <{tag}>")]
    XmlAttributeMissing { attr: String, tag: String },
    #[error(
        "could not parse attribute {attr}=\"{content}\" on <{tag}> as {expected_type}; {source}"
    )]
    InvalidAttribute {
        attr: String,
        tag: String,
        content: String,
        source: Box<dyn std::error::Error>,
        expected_type: String,
    },
    #[error("unexpected node <{0}>")]
    UnexpectedXmlNode(String),
    #[error("duplicate Geometry name '{0}'")]
    DuplicateGeometryName(String),
    #[error(
        "duplicate DMXBreak attribute {duplicate_break} in GeometryReference '{geometry_reference_name}'"
    )]
    DuplicateDmxBreak {
        duplicate_break: u16,
        geometry_reference_name: String,
    },
    #[error("unexpected GeometryReference '{0}' as top-level Geometry")]
    UnexpectedTopLevelGeometryReference(String),
    #[error("unknown Geometry '{0}' referenced")]
    UnknownGeometry(String),
    #[error(
        "non-top-level Geometry '{target}' referenced in GeometryReference '{geometry_reference}'"
    )]
    NonTopLevelGeometryReferenced {
        target: String,
        geometry_reference: String,
    },
}

impl ProblemType {
    pub(crate) fn at(self, node: &roxmltree::Node) -> Problem {
        Problem {
            p: self,
            at: node.position(),
        }
    }
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
