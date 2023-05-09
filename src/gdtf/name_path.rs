use std::{fmt::Display, str::FromStr};

use super::name::{Name, NameError};

/// called "Node" in the GDTF 1.2 standard
#[derive(Default)]
pub struct NamePath(Vec<Name>);

impl FromStr for NamePath {
    type Err = NameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut out = NamePath(vec![]);
        for n in s.split('.') {
            match Name::try_from(n) {
                Ok(name) => out.0.push(name),
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}

impl Display for NamePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for name in self.0.iter() {
            s.push_str(name.as_str());
            s.push('.');
        }
        s.pop();
        write!(f, "{s}")?;
        Ok(())
    }
}
