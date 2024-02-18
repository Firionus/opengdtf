use crate::{
    low_level::{self, BasicGeometry, FixtureType, Geometries, LowLevelGdtf},
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
    geometries: &mut Vec<low_level::GeometryType>,
    mut iter: impl Iterator<Item = &'a Geometry>,
) {
    for g in iter {
        let low_level = match &g.t {
            crate::GeometryType::Geometry { children } => {
                let mut c = Vec::<low_level::GeometryType>::new();
                recusively_add_geometries(&mut c, children.iter());

                low_level::GeometryType::Geometry {
                    basic: BasicGeometry {
                        name: g.name.clone(),
                        model: None, // TODO
                    },
                    children: c,
                }
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
                        dmx_break: dmx_break.clone(),
                    })
                    .collect::<Vec<low_level::Break>>();
                breaks.push(low_level::Break {
                    dmx_offset: overwrite.1.clone(),
                    dmx_break: overwrite.0,
                });

                low_level::GeometryType::GeometryReference {
                    basic: BasicGeometry {
                        name: g.name.clone(),
                        model: None, // TODO
                    },
                    geometry: geometry.clone(),
                    breaks,
                }
            }
        };
        geometries.push(low_level);
    }
}
