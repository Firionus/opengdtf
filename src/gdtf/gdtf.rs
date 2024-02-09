use serde::{Deserialize, Serialize};

use super::data_version::DataVersion;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Gdtf {
    pub data_version: DataVersion,
}
