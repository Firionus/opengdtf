use std::fmt::Display;

use crate::{ProblemAt, Problems};

pub(crate) trait AssignOrHandle<T: Display> {
    fn assign_or_handle(self, to: &mut T, problems: &mut Problems);
}

impl<T: Display> AssignOrHandle<T> for Result<T, ProblemAt> {
    /// Assigns the value, or handles the problem by keeping the pre-assigned
    /// default.
    fn assign_or_handle(self, to: &mut T, problems: &mut Problems) {
        match self {
            Ok(v) => *to = v,
            Err(p) => p.handled_by(format!("using default {}", to), problems),
        }
    }
}
