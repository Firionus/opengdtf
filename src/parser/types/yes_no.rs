#[derive(strum::Display, strum::EnumString)]
pub(crate) enum YesNoEnum {
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
