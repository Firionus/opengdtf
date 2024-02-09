//! The problems system is the core error handling mechanism in the GDTF parser.
//! See the unit tests of this module for an example of how to do it.

use std::str::from_utf8;

use quick_xml::Reader;

pub type Problems = Vec<HandledProblem>;

/// A recoverable problem in a GDTF file, with position information and info on
/// the action taken to recover from it.
#[derive(thiserror::Error, Debug)]
#[error("{p}; {action}")]
pub struct HandledProblem {
    p: Problem,
    pub action: String,
}

/// A recoverable kind of problem in a GDTF file.
#[derive(thiserror::Error, Debug)]
pub enum Problem {
    #[error("invalid XML at line {1}. caused by {0}")]
    InvalidXml(quick_xml::Error, DocumentPosition),
    #[error(
        "unexpected condition occured. This is a fault in opengdtf. \
        Please open an issue at https://github.com/Firionus/opengdtf/issues/new. Caused by: {0}"
    )]
    Unexpected(Box<dyn std::error::Error>),
}

#[derive(derive_more::Display, Debug, Default)]
#[display(fmt = "{line}:{column}")]
pub struct DocumentPosition {
    line: u32,
    column: u32,
}

/// Get line/col position of reader
///
/// Determing line/col position quickly in quick-xml is a long standing
/// issue. Please track https://github.com/tafia/quick-xml/issues/109
///
/// In case of failure, this pushes onto the problem vector.
pub(crate) fn pos(
    reader: Reader<&[u8]>,
    problems_provider: &mut impl ProblemsMut,
) -> DocumentPosition {
    let end_pos = reader.buffer_position();
    let s = match from_utf8(&reader.get_ref()[0..end_pos])
        .unexpected("returning 0:0", problems_provider)
    {
        Some(s) => s,
        None => return DocumentPosition::default(),
    };
    let mut line = 1u32;
    let mut column = 0u32;
    for c in s.chars() {
        if c == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    DocumentPosition { line, column }
}

// fast way to handle unexpected problems
pub(crate) trait HandleUnexpected<T> {
    fn unexpected(self, action: &str, problems_provider: &mut impl ProblemsMut) -> Option<T>;
}

impl<T, E: Into<Box<dyn std::error::Error>>> HandleUnexpected<T> for Result<T, E> {
    fn unexpected(self, action: &str, problems_provider: &mut impl ProblemsMut) -> Option<T> {
        todo!() // TODO
    }
}

/// Implementors can provide a mutable reference to a Problems vector.
///
/// This shortens error handling code that must push onto the problem vector,
/// since they don't have to write out the borrow and field access for owned or
/// more complex types.
pub(crate) trait ProblemsMut {
    fn problems_mut(&mut self) -> &mut Problems;
}

impl ProblemsMut for Problems {
    fn problems_mut(&mut self) -> &mut Problems {
        self
    }
}

impl Problem {
    /// Specify what action was taken to resolve the problem and then push it
    /// onto the problems.
    pub(crate) fn handle<T: Into<String>>(
        self,
        action: T,
        problems_provider: &mut impl ProblemsMut,
    ) {
        problems_provider.problems_mut().push(HandledProblem {
            p: self,
            action: action.into(),
        });
    }
}
