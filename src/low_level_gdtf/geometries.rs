use std::num::NonZeroU8;

use derivative::Derivative;
use serde::Serialize;

use crate::{dmx_address::DmxAddress, name::Name};

#[derive(Serialize, Debug, Default, PartialEq)]
pub struct Geometries {
    #[serde(default, rename = "$value")]
    pub children: Vec<GeometryType>,
}

#[derive(Serialize, Debug, PartialEq)]
pub enum GeometryType {
    Geometry {
        #[serde(flatten)]
        basic: BasicGeometry,
        #[serde(default, rename = "$value")]
        children: Vec<GeometryType>,
    },
    GeometryReference {
        #[serde(flatten)]
        basic: BasicGeometry,
        #[serde(rename = "@Geometry")]
        geometry: Name,
        #[serde(default, rename = "Break")]
        breaks: Vec<Break>,
    },
}

#[derive(Serialize, Debug, PartialEq)]
pub struct BasicGeometry {
    #[serde(rename = "@Name")]
    pub name: Name,
    #[serde(rename = "@Model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Name>,
    // position: Matrix, // TODO
}

#[derive(Derivative, Serialize, Debug, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "PascalCase")]
pub struct Break {
    #[serde(rename = "@DMXOffset")]
    pub dmx_offset: DmxAddress,
    /// The GDTF builder does not really allow one to use a DMXBreak of 0, it
    /// always somehow changes back to 1. Also, 1 byte size is specified in the
    /// DIN. Therefore, use NonZeroU8.
    #[derivative(Default(value = "NonZeroU8::MIN"))]
    #[serde(rename = "@DMXBreak")]
    pub dmx_break: NonZeroU8,
}

pub fn count_geometry_children(children: &[GeometryType]) -> u64 {
    children.iter().fold(0, |i, g| {
        if let GeometryType::Geometry { children, .. } = g {
            i + count_geometry_children(children) + 1
        } else {
            i
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{low_level_gdtf::low_level_gdtf::LowLevelGdtf, parse::parse_geometry_children};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_geometries_serialization_and_parsing() {
        let mut gdtf = LowLevelGdtf::default();
        gdtf.fixture_type
            .geometries
            .children
            .push(GeometryType::Geometry {
                basic: BasicGeometry {
                    name: "Test".try_into().unwrap(),
                    model: Some("Test".try_into().unwrap()),
                },
                children: Vec::new(),
            });
        match gdtf.fixture_type.geometries.children.first_mut().unwrap() {
            GeometryType::Geometry {
                ref mut children, ..
            } => children.push(GeometryType::GeometryReference {
                basic: BasicGeometry {
                    name: "Second level".try_into().unwrap(),
                    model: None,
                },
                geometry: "referencee".try_into().unwrap(),
                breaks: Vec::from([
                    Break {
                        dmx_offset: 1.try_into().unwrap(),
                        dmx_break: 1.try_into().unwrap(),
                    },
                    Break {
                        dmx_offset: 2.try_into().unwrap(),
                        dmx_break: 1.try_into().unwrap(),
                    },
                ]),
            }),
            _ => unreachable!(),
        };
        let expected = concat!(
            r#"<GDTF DataVersion="1.2"><FixtureType Name="" ShortName="" LongName="" Manufacturer="" Description="" FixtureTypeID="00000000-0000-0000-0000-000000000000" RefFT="" CanHaveChildren="Yes">"#,
            r#"<Geometries><Geometry Name="Test" Model="Test"><GeometryReference Name="Second level" Geometry="referencee">"#,
            r#"<Break DMXOffset="1" DMXBreak="1"/><Break DMXOffset="2" DMXBreak="1"/></GeometryReference>"#,
            r#"</Geometry></Geometries></FixtureType></GDTF>"#
        );
        assert_eq!(quick_xml::se::to_string(&gdtf).unwrap(), expected);

        let doc = roxmltree::Document::parse(expected).unwrap();
        let geometries = doc
            .descendants()
            .find(|n| n.has_tag_name("Geometries"))
            .unwrap();
        let mut p = Vec::new();
        assert_eq!(
            parse_geometry_children(&mut p, geometries).collect::<Vec<_>>(),
            gdtf.fixture_type.geometries.children
        );
    }
}
