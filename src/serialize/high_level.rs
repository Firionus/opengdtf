use crate::{
    low_level::{FixtureType, Geometries, LowLevelGdtf},
    Gdtf, SerializationError,
};

impl Gdtf {
    pub fn serialize(&self) -> Result<Vec<u8>, SerializationError> {
        let llgdtf = LowLevelGdtf {
            data_version: self.data_version.to_owned(),
            fixture_type: FixtureType {
                name: self.name.to_owned(),
                short_name: self.short_name.to_owned(),
                long_name: self.long_name.to_owned(),
                manufacturer: self.manufacturer.to_owned(),
                description: self.description.to_owned(),
                id: self.fixture_type_id.to_owned(),
                ref_ft: self.ref_ft,
                can_have_children: self.can_have_children.into(),
                geometries: Geometries::default(), // TODO
            },
        };
        llgdtf.serialize()
    }
}
