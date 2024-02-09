use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Represents the GDTF DataVersion
///
/// We first use strum to derive FromStr and Display, then serde_with to derive
/// Serialize/Deserialize based on that.
#[derive(
    Debug, PartialEq, strum::EnumString, strum::Display, SerializeDisplay, DeserializeFromStr,
)]
pub enum DataVersion {
    #[strum(to_string = "1.0")]
    V1_0,
    #[strum(to_string = "1.1")]
    V1_1,
    #[strum(to_string = "1.2")]
    V1_2,
}
