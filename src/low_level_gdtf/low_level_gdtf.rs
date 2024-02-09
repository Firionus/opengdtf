use serde::Serialize;

use crate::gdtf::data_version::DataVersion;

#[derive(Serialize, Debug)]
pub struct LowLevelGdtf {
    #[serde(rename = "@DataVersion")]
    data_version: DataVersion,
}
