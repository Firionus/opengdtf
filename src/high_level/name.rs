use std::str::FromStr;

use derive_more::{DebugCustom, Display};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// GDTF Name type
///
/// A Name is a UTF-8 String with restricted characters. According to  DIN SPEC
/// 15800:2022-02 Annex C, the disallowed Unicode code points are:
/// - U+0000..=U+001F (<control>)
/// - U+0021 (!)
/// - U+0024 ($)
/// - U+0026 (&)
/// - U+002C (,)
/// - U+002E (.)
/// - U+003F (?)
/// - U+005B..=U+005E ([\]^)
/// - U+007B..=U+007F ({|}~<control>)
#[derive(
    PartialOrd,
    PartialEq,
    Eq,
    Ord,
    Clone,
    Hash,
    Display,
    DebugCustom,
    Default,
    Serialize,
    Deserialize,
)]
pub struct Name(String);

impl TryFrom<&str> for Name {
    type Error = NameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut invalid_chars = String::new();

        let name = Self(
            value
                .chars()
                .map(|c| match c {
                    '!'
                    | '$'
                    | '&'
                    | ','
                    | '.'
                    | '?'
                    | '\x00'..='\x1f'
                    | '['..='^'
                    | '{'..='\x7f' => {
                        invalid_chars.push(c);
                        '□'
                    }
                    _ => c,
                })
                .collect::<String>(),
        );

        if invalid_chars.is_empty() {
            Ok(name)
        } else {
            Err(NameError {
                fixed: name,
                invalid_chars,
            })
        }
    }
}

impl TryFrom<String> for Name {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl FromStr for Name {
    type Err = NameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.try_into()
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

pub(crate) trait IntoValidName {
    fn into_valid(self) -> Name;
}

impl IntoValidName for &str {
    /// Creates a Name from self, with invalid chars replaced by '□'
    fn into_valid(self) -> Name {
        self.try_into().unwrap_or_else(|e: NameError| e.fixed)
    }
}

impl Name {
    /// Construct the default name based on the XML tag name and the 0-based XML
    /// node index in its parent.
    ///
    /// In case of invalid characters, the characters in question are replaced
    /// with a tofu character and returned for error reporting.
    pub fn default<T: Into<String>>(
        tag: T,
        xml_node_index_in_parent: usize,
    ) -> Result<Name, NameError> {
        format!("{} {}", tag.into(), xml_node_index_in_parent + 1).try_into()
    }

    /// Construct the default name based on the XML tag name and the 0-based XML
    /// node index in its parent.
    ///
    /// Invalid characters are replaced with '□'. For explicit error handling, see [`Name::default`].
    pub fn valid_default<T: Into<String>>(tag: T, xml_node_index_in_parent: usize) -> Name {
        format!("{} {}", tag.into(), xml_node_index_in_parent + 1).into_valid()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Error, Debug)]
#[error("invalid GDTF Name type due to chars '{invalid_chars}'; replaced with '□'")]
pub struct NameError {
    /// Name where all invalid chars were replaced with '□'
    pub fixed: Name,
    pub invalid_chars: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        Name::from_str("Hello World").unwrap();
        Name::try_from("Hello World").unwrap();
        Name::try_from("Hello World".to_string()).unwrap();

        assert!(matches!(
            Name::try_from("a.b"),
            Err(NameError {
                fixed,
                invalid_chars,
            }) if fixed == "a□b" && invalid_chars == "."
        ));
        assert_eq!("a~b".into_valid(), "a□b");
        assert!(matches!(
            Name::try_from("a]b"),
            Err(NameError {
                fixed,
                invalid_chars,
            }) if fixed == "a□b" && invalid_chars == "]"
        ));

        assert_eq!("yay", format!("{}", Name::try_from("yay").unwrap()));
        assert_eq!("\"yay\"", format!("{:?}", Name::try_from("yay").unwrap()));
        assert_eq!("\"yay\"", format!("{:#?}", Name::try_from("yay").unwrap()));
    }
}
