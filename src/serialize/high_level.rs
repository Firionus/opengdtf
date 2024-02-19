use crate::{
    low_level::{self, BasicGeometry, FixtureType, Geometries, LowLevelGdtf, LowLevelGeometryType},
    Gdtf, Geometry, SerializationError,
};

impl Gdtf {
    pub fn serialize(&self) -> Result<Vec<u8>, SerializationError> {
        let mut geometries = Geometries::default();
        recusively_add_geometries(&mut geometries.children, self.geometries().iter());

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
                geometries,
            },
        };
        llgdtf.serialize()
    }
}

fn recusively_add_geometries<'a>(
    geometries: &mut Vec<LowLevelGeometryType>,
    iter: impl Iterator<Item = &'a Geometry>,
) {
    for g in iter {
        let basic = BasicGeometry {
            name: g.name.clone(),
            model: None, // TODO model, position
        };
        let low_level = match &g.t {
            crate::GeometryType::Geometry { children } => {
                let mut c = Vec::<LowLevelGeometryType>::new();
                recusively_add_geometries(&mut c, children.iter());

                LowLevelGeometryType::Geometry { basic, children: c }
            }
            crate::GeometryType::GeometryReference {
                geometry,
                overwrite,
                offsets,
            } => {
                let mut breaks = offsets
                    .iter()
                    .map(|(dmx_break, dmx_address)| low_level::Break {
                        dmx_offset: dmx_address.clone(),
                        dmx_break: *dmx_break,
                    })
                    .collect::<Vec<low_level::Break>>();
                breaks.push(low_level::Break {
                    dmx_offset: overwrite.1.clone(),
                    dmx_break: overwrite.0,
                });

                // TODO with Breaks in GeometryReferences, we must ensure that for every value
                // of Break in channels of the referenced geometry, there is exactly one break
                // entry, not more or less. GDTF does not allow too many breaks to be present,
                // whereas this is allowed in high-level gdtf to make it constructible. Unused
                // ones can just be removed.

                LowLevelGeometryType::GeometryReference {
                    basic,
                    geometry: geometry.clone(),
                    breaks,
                }
            }
        };
        geometries.push(low_level);
    }
}
