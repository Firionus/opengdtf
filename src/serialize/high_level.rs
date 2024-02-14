use crate::{
    low_level::{FixtureType, Geometries, LowLevelGdtf},
    Gdtf, SerializationError,
};

use super::low_level;

// TODO turn into impl on &Gdtf?
pub fn serialize(gdtf: &Gdtf) -> Result<Vec<u8>, SerializationError> {
    let llgdtf = LowLevelGdtf {
        data_version: gdtf.data_version.to_owned(),
        fixture_type: FixtureType {
            name: gdtf.name.to_owned(),
            short_name: gdtf.short_name.to_owned(),
            long_name: gdtf.long_name.to_owned(),
            manufacturer: gdtf.manufacturer.to_owned(),
            description: gdtf.description.to_owned(),
            id: gdtf.fixture_type_id.to_owned(),
            ref_ft: gdtf.ref_ft,
            can_have_children: gdtf.can_have_children.into(),
            geometries: Geometries::default(), // TODO
        },
    };
    low_level::serialize(&llgdtf)
}
