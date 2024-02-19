use serde::Serialize;
use uuid::Uuid;

use crate::{low_level, DataVersion, Name};

use super::YesNoEnum;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename = "GDTF")]
pub struct LowLevelGdtf {
    #[serde(rename = "@DataVersion")]
    pub data_version: DataVersion,
    #[serde(rename = "FixtureType")]
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

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase")]
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
    #[serde(rename = "@FixtureTypeID")]
    pub id: Uuid,
    // Not implemented: Thumbnail, ThumbnailOffsetX, ThumbnailOffsetY
    #[serde(rename = "@RefFT")]
    pub ref_ft: Option<Uuid>,
    #[serde(rename = "@CanHaveChildren")]
    pub can_have_children: YesNoEnum,
    #[serde(default)]
    pub geometries: low_level::Geometries,
}
