use std::num::ParseIntError;
use std::str::FromStr;

// TODO we could remove all of this and replace with u16 if we just used 0-based breaks instead of 1-based

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
        s.parse::<u16>().map_err(BreakError::from)?.try_into()
    }
}

impl Default for Break {
    fn default() -> Self {
        Self(1)
    }
}

impl Break {
    pub fn value(&self) -> &u16 {
        &self.0
    }
}

#[derive(thiserror::Error, Debug)]
pub enum BreakError {
    #[error("DMX breaks of value 0 are not allowed")]
    ZeroBreak(),
    #[error("could not parse as valid integer: {source}")]
    NonIntegerBreak {
        #[from]
        source: ParseIntError,
    },
}
