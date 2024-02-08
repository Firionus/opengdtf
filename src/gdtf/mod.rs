use getset::Getters;
use uuid::Uuid;

use self::{data_version::DataVersion, dmx_modes::DmxMode, geometries::Geometries, name::Name};

pub mod channel_offsets;
pub mod checked_graph;
pub mod data_version;
pub mod dmx_break;
pub mod dmx_modes;
pub mod geometries;
pub mod geometry;
pub mod name;

/// A mid-level representation of a GDTF fixture.
///
/// This aims to be:
/// - Constructed piece by piece
/// - "Valid" at any construction step, in the sense of uniquely correlating to
///   a valid and sensible GDTF file
/// - Serializable to a GDTF file, Parseable from a GDTF file
///
/// As a mid-level representation, it does not have to completely copy the
/// structure of GDTF but has to maintain enough information to be serializable
/// to GDTF.  
/// For example, template channels and geometries are kept as such and not
/// instantiated. Yet, references between nodes don't have to be kept as strings
/// but can be encoded with indices or graphs instead.
#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct Gdtf {
    pub data_version: DataVersion,
    pub fixture_type_id: Uuid,
    pub ref_ft: Option<Uuid>,
    pub can_have_children: bool,

    pub name: Name,
    pub short_name: String,
    pub long_name: String,
    pub manufacturer: String,
    pub description: String,

    pub geometries: Geometries,

    dmx_modes: Vec<DmxMode>,
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

impl Gdtf {
    pub fn dmx_mode(&self, index: usize) -> Result<&DmxMode, GdtfError> {
        self.dmx_modes
            .get(index)
            .ok_or(GdtfError::InvalidDmxModeIndex())
    }

    pub fn dmx_mode_mut(&mut self, index: usize) -> Result<&mut DmxMode, GdtfError> {
        self.dmx_modes
            .get_mut(index)
            .ok_or(GdtfError::InvalidDmxModeIndex())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GdtfError {
    #[error("Invalid DMX mode index")]
    InvalidDmxModeIndex(),
}
