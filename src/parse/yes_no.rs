use serde_with::{DeserializeFromStr, SerializeDisplay};

// TODO this should be in low_level module
#[derive(
    Debug,
    strum::Display,
    strum::EnumString,
    SerializeDisplay,
    DeserializeFromStr,
    Clone,
    PartialEq,
    Default,
)]
pub enum YesNoEnum {
    #[strum(to_string = "Yes")]
    #[default] // this works for CanHaveChildren attribute
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
