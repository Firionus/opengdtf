use serde::Serialize;

use crate::{gdtf::data_version::DataVersion, name::Name};

#[derive(Serialize, Debug)]
#[serde(rename = "GDTF")]
pub struct LowLevelGdtf {
    #[serde(rename = "@DataVersion")]
    pub data_version: DataVersion,

    pub fixture_type: FixtureType,
}

impl Default for LowLevelGdtf {
    fn default() -> Self {
        LowLevelGdtf {
            data_version: DataVersion::V1_2,
            fixture_type: FixtureType::default(),
        }
    }
}

#[derive(Serialize, Debug, Default)]
pub struct FixtureType {
    #[serde(rename = "@Name")]
    pub name: Name,
    #[serde(rename = "@ShortName")]
    pub short_name: String,
    #[serde(rename = "@LongName")]
    pub long_name: String,
    #[serde(rename = "@Manufacturer")]
    pub manufacturer: String,
    #[serde(rename = "@Description")]
    pub description: String,
}
