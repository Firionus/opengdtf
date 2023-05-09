#[derive(Debug, PartialEq, strum::EnumString, strum::Display)]
pub enum DataVersion {
    #[strum(to_string = "1.0")]
    V1_0,
    #[strum(to_string = "1.1")]
    V1_1,
    #[strum(to_string = "1.2")]
    V1_2,
}
