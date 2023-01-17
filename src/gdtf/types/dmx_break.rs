use std::str::FromStr;

use crate::gdtf::errors::BreakError;

/// DMX Break, which is an unsigned integer bigger than 0
#[derive(derive_more::Display, derive_more::DebugCustom, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Break(u16);

impl TryFrom<u16> for Break {
    type Error = BreakError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if value == 0 {
            Err(BreakError::ZeroBreak())
        } else {
            Ok(Break(value))
        }
    }
}

impl FromStr for Break {
    type Err = BreakError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u16>()
            .map_err(|err| BreakError::from(err))?
            .try_into()
    }
}
