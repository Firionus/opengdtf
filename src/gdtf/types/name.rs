use std::{fmt::Debug, fmt::Display, str::FromStr};

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
#[derive(PartialOrd, PartialEq, Eq, Ord, Clone, Hash)]
pub struct Name(String);

#[derive(Error, Debug)]
#[error("invalid GDTF Name type due to chars '{invalid_chars}'; replaced with '□'")]
pub struct NameError {
    /// Name where all invalid chars were replaced with '□'
    pub name: Name,
    pub invalid_chars: String,
}

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
                name,
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

// TODO replace with a macro like derive_more, ...
impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Name {
    /// construct the default name based on the XML tag name and the 0-based XML
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

    pub fn as_str(&self) -> &str {
        &self.0
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
                name,
                invalid_chars,
            }) if name == "a□b" && invalid_chars == "."
        ));

        assert!(matches!(
            Name::try_from("a]b"),
            Err(NameError {
                name,
                invalid_chars,
            }) if name == "a□b" && invalid_chars == "]"
        ));

        assert_eq!("yay", format!("{}", Name::try_from("yay").unwrap()));
        assert_eq!("yay", format!("{:?}", Name::try_from("yay").unwrap()));
        assert_eq!("yay", format!("{:#?}", Name::try_from("yay").unwrap()));
    }
}
