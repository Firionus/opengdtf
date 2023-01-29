use uuid::Uuid;

use self::{dmx_modes::DmxMode, geometries::Geometries, types::data_version::DataVersion};

pub mod dmx_modes;
pub mod geometries;
pub mod geometry;
pub mod types;

#[derive(Debug)]
pub struct Gdtf {
    pub data_version: DataVersion,
    pub fixture_type_id: Uuid,
    pub ref_ft: Option<Uuid>,
    pub can_have_children: bool,

    pub name: String, // TODO should be Name type
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
