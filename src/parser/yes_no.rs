use serde_with::{DeserializeFromStr, SerializeDisplay};

#[derive(
    Debug, strum::Display, strum::EnumString, SerializeDisplay, DeserializeFromStr, Clone, PartialEq,
)]
pub enum YesNoEnum {
    #[strum(to_string = "Yes")]
    Yes,
    #[strum(to_string = "No")]
    No,
}

impl From<YesNoEnum> for bool {
    fn from(value: YesNoEnum) -> Self {
        match value {
            YesNoEnum::Yes => true,
            YesNoEnum::No => false,
        }
    }
}

impl From<bool> for YesNoEnum {
    fn from(value: bool) -> Self {
        match value {
            true => YesNoEnum::Yes,
            false => YesNoEnum::No,
        }
    }
}

/// Default "Yes" works for CanHaveChildren attribute
impl Default for YesNoEnum {
    fn default() -> Self {
        Self::Yes
    }
}
