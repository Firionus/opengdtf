//! The problems system is the core error handling mechanism in the GDTF parser.
//! See the unit tests of this module for an example of how to do it.

use roxmltree::TextPos;

use crate::types::{dmx_break::Break, name::Name};

use super::utils::XmlPosition;

pub type Problems = Vec<HandledProblem>;

/// A recoverable problem in a GDTF file, with position information and info on
/// the action taken to recover from it.
#[derive(thiserror::Error, Debug)]
#[error("{p}; {action}")]
pub struct HandledProblem {
    p: Problem,
    pub action: String,
}

/// A recoverable problem in a GDTF file, with position information.
#[derive(thiserror::Error, Debug)]
#[error("{p} (line {at})")]
pub struct Problem {
    p: ProblemType,
    at: TextPos,
}

/// A recoverable kind problem in a GDTF file.
#[derive(thiserror::Error, Debug)]
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
    #[error(
        "non-top-level Geometry '{target}' referenced in GeometryReference '{geometry_reference}'"
    )]
    NonTopLevelGeometryReferenced {
        target: Name,
        geometry_reference: Name,
    },
    #[error(
        "unexpected condition occured. This is a fault in opengdtf. \
    Please open an issue at https://github.com/Firionus/opengdtf/issues/new. Caused by: {0}"
    )]
    Unexpected(String),
}

impl ProblemType {
    /// Add position information to problem based on Node where it occured.
    pub(crate) fn at(self, node: &roxmltree::Node) -> Problem {
        Problem {
            p: self,
            at: node.position(),
        }
    }
}

pub(crate) trait HandleProblem<T, S: Into<String>> {
    fn handled_by(self, action: S, problems: &mut Problems) -> Option<T>;
}

impl Problem {
    /// Specify what action was taken to resolve the problem and then push it
    /// onto the problems.
    pub fn handled_by<T: Into<String>>(self, action: T, problems: &mut Problems) {
        problems.push(HandledProblem {
            p: self,
            action: action.into(),
        });
    }
}

impl<T, S: Into<String>> HandleProblem<T, S> for Result<T, Problem> {
    /// Specify what action will be taken to resolve a possible Err(Problem),
    /// push it onto problems and return None. If the result is Ok(v), Some(v)
    /// is returned instead.
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

impl HandledProblem {
    pub fn problem_type(&self) -> &ProblemType {
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

        ProblemType::UnexpectedXmlNode("whatsThis".into())
            .at(&node)
            .handled_by("ignoring node", &mut problems);

        assert!(matches!(
            &problems[0],
            HandledProblem {
                action,
                p: Problem {
                    at,
                    p: ProblemType::UnexpectedXmlNode(..)
                }
            } if action == "ignoring node" && at == &TextPos{row: 1, col: 1}
        ))
    }
}
