use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::name::Name;

use super::data_version::DataVersion;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Gdtf {
    pub data_version: DataVersion,

    pub name: Name,
    pub short_name: String,
    pub long_name: String,
    pub manufacturer: String,
    pub description: String,

    pub fixture_type_id: Uuid,
}
