use strum::EnumString;
use uuid::Uuid;

use self::{dmx_modes::DmxMode, geometries::Geometries};

pub mod dmx_modes;
mod errors;
pub mod geometries;
pub mod types;

#[derive(Debug)]
pub struct Gdtf {
    pub data_version: DataVersion,
    pub fixture_type_id: Uuid,
    pub ref_ft: Option<Uuid>,
    pub can_have_children: bool,

    pub name: String,
    pub short_name: String,
    pub long_name: String,
    pub manufacturer: String,
    pub description: String,

    pub geometries: Geometries,

    pub dmx_modes: Vec<DmxMode>,
}

impl Default for Gdtf {
    fn default() -> Self {
        Self {
            data_version: DataVersion::V1_2,
            fixture_type_id: Uuid::nil(),
            ref_ft: None,
            can_have_children: true,
            name: Default::default(),
            short_name: Default::default(),
            long_name: Default::default(),
            manufacturer: Default::default(),
            description: Default::default(),
            geometries: Default::default(),
            dmx_modes: Default::default(),
        }
    }
}

#[derive(Debug, EnumString, PartialEq, strum::Display)]
pub enum DataVersion {
    #[strum(to_string = "1.0")]
    V1_0,
    #[strum(to_string = "1.1")]
    V1_1,
    #[strum(to_string = "1.2")]
    V1_2,
}
