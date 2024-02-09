use serde::Serialize;

use crate::gdtf::data_version::DataVersion;

#[derive(Serialize, Debug)]
pub struct LowLevelGdtf {
    #[serde(rename = "@DataVersion")]
    pub data_version: DataVersion,
}

impl Default for LowLevelGdtf {
    fn default() -> Self {
        LowLevelGdtf {
            data_version: DataVersion::V1_2,
        }
    }
}
