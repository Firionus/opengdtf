//! The problems system is the core error handling mechanism in the GDTF parser.
//! See the unit tests of this module for an example of how to do it.

use roxmltree::{Node, TextPos};

use crate::{
    geometries::GeometriesError,
    types::{dmx_break::Break, name::Name},
};

pub type Problems = Vec<HandledProblem>;

/// A recoverable problem in a GDTF file, with position information and info on
/// the action taken to recover from it.
#[derive(thiserror::Error, Debug)]
#[error("{p}; {action}")]
pub struct HandledProblem {
    p: ProblemAt,
    pub action: String,
}

/// A recoverable problem in a GDTF file, with position information.
#[derive(thiserror::Error, Debug)]
#[error("{p} (line {at})")]
pub struct ProblemAt {
    p: Problem,
    at: TextPos,
}

/// A recoverable kind of problem in a GDTF file.
#[derive(thiserror::Error, Debug)]
pub enum Problem {
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
    DuplicateGeometryName(Name),
    #[error(
        "duplicate DMXBreak attribute {duplicate_break} in GeometryReference '{geometry_reference}'"
    )]
    DuplicateDmxBreak {
        duplicate_break: Break,
        geometry_reference: Name,
    },
    #[error("unexpected GeometryReference '{0}' as top-level Geometry")]
    UnexpectedTopLevelGeometryReference(Name),
    #[error("unknown Geometry '{0}' referenced")]
    UnknownGeometry(Name),
    #[error("invalid GeometryReference: {0}")]
    InvalidGeometryReference(GeometriesError),
    #[error("geometry '{geometry}' of DMX mode '{mode}' is not top level")]
    NonTopLevelDmxModeGeometry { geometry: Name, mode: Name },
    #[error("got {0} bytes for channel but only up to 4 are supported")]
    UnsupportedByteCount(usize),
    #[error("channel functions with mode master {mode_master} from {mode_from} to {mode_to} don't have an \
    unambiguous DMXTo value because not every DMXFrom value occurs the same amount of times")]
    AmbiguousDmxFrom {
        mode_master: String,
        mode_from: u32,
        mode_to: u32,
    },
    #[error("ModeFrom or ModeTo missing on channel function '{0}' with ModeMaster")]
    MissingModeFromOrTo(String),
    #[error("channel with name {0} not found in mode {1}")]
    UnknownChannel(Name, Name),
    #[error("channel function with name {name} not found in mode {mode}")]
    UnknownChannelFunction { name: Name, mode: Name },
    #[error(
        "unexpected condition occured. This is a fault in opengdtf. \
        Please open an issue at https://github.com/Firionus/opengdtf/issues/new. Caused by: {0}"
    )]
    Unexpected(String),
}

impl Problem {
    /// Add position information to problem based on Node where it occured.
    pub(crate) fn at(self, node: &Node) -> ProblemAt {
        ProblemAt {
            p: self,
            at: node.document().text_pos_at(node.position()),
        }
    }
}

impl ProblemAt {
    /// Specify what action was taken to resolve the problem and then push it
    /// onto the problems.
    pub fn handled_by<T: Into<String>>(self, action: T, problems: &mut Problems) {
        problems.push(HandledProblem {
            p: self,
            action: action.into(),
        });
    }

    pub fn problem(&self) -> &Problem {
        &self.p
    }
}

pub(crate) trait HandleProblem<T, S: Into<String>> {
    fn ok_or_handled_by(self, action: S, problems: &mut Problems) -> Option<T>;
}

impl<T, S: Into<String>> HandleProblem<T, S> for Result<T, ProblemAt> {
    /// Specify what action will be taken to resolve a possible Err(Problem),
    /// push it onto problems and return None. If the result is Ok(v), Some(v)
    /// is returned instead.
    fn ok_or_handled_by(self, action: S, problems: &mut Problems) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(p) => {
                p.handled_by(action, problems);
                None
            }
        }
    }
}

impl HandledProblem {
    pub fn problem(&self) -> &Problem {
        &self.p.p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_of_error_handling_in_parser() {
        let mut problems = Problems::new(); // global problems vector

        let binding = roxmltree::Document::parse(r#"<whatsThis />"#).unwrap();
        let node = binding.root_element();

        // encounter a problem
        Problem::UnexpectedXmlNode("whatsThis".into())
            .at(&node)
            .handled_by("ignoring node", &mut problems);

        assert!(matches!(
            problems.first().unwrap(),
            HandledProblem {
                action,
                p: ProblemAt {
                    at,
                    p: Problem::UnexpectedXmlNode(..)
                }
            } if action == "ignoring node" && at == &TextPos{row: 1, col: 1}
        ))
    }
}
