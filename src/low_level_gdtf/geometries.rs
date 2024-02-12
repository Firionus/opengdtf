use std::num::NonZeroU8;

use derivative::Derivative;
use serde::Serialize;

use crate::{dmx_address::DmxAddress, name::Name};

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Geometries {
    #[serde(default, rename = "$value")]
    children: Vec<GeometryType>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GeometryType {
    Geometry {
        #[serde(rename = "@Name")]
        name: Name,
        #[serde(rename = "@Model")]
        model: Name,
        // position: Matrix, // TODO
        #[serde(default, rename = "$value")]
        children: Vec<GeometryType>,
    },
    GeometryReference {
        #[serde(rename = "@Name")]
        name: Name,
        #[serde(rename = "@Model")]
        model: Name,
        // position: Matrix, // TODO
        #[serde(rename = "@Geometry")]
        geometry: Name,
        #[serde(default, rename = "$value")]
        children: Vec<Break>,
    },
}

#[derive(Derivative, Serialize, Debug, PartialEq)]
#[derivative(Default)]
#[serde(rename_all = "PascalCase")]
pub struct Break {
    #[serde(rename = "@DMXOffset")]
    dmx_offset: DmxAddress,
    /// The GDTF builder does not really allow one to use a DMXBreak of 0, it
    /// always somehow changes back to 1. Also, 1 byte size is specified in the
    /// DIN. Therefore, use NonZeroU8.
    #[derivative(Default(value = "NonZeroU8::MIN"))]
    #[serde(rename = "@DMXBreak")]
    dmx_break: NonZeroU8,
}

#[cfg(test)]
mod tests {
    use crate::low_level_gdtf::low_level_gdtf::LowLevelGdtf;

    use super::*;

    #[test]
    fn test_geometries_serialization() {
        let mut gdtf = LowLevelGdtf::default();
        gdtf.fixture_type
            .geometries
            .children
            .push(GeometryType::Geometry {
                name: "Test".try_into().unwrap(),
                model: "Test".try_into().unwrap(),
                children: Vec::new(),
            });
        match gdtf.fixture_type.geometries.children.first_mut().unwrap() {
            GeometryType::Geometry {
                ref mut children, ..
            } => children.push(GeometryType::Geometry {
                name: "Second level".try_into().unwrap(),
                model: "Yolo".try_into().unwrap(),
                children: Vec::new(),
            }),
            _ => unreachable!(),
        };
        // println!("{}", quick_xml::se::to_string(&gdtf).unwrap());
        assert_eq!(
            quick_xml::se::to_string(&gdtf).unwrap(),
            concat!(
                r#"<GDTF DataVersion="1.2"><FixtureType Name="" ShortName="" LongName="" Manufacturer="" Description="" FixtureTypeID="00000000-0000-0000-0000-000000000000" RefFT="" CanHaveChildren="Yes">"#,
                r#"<Geometries><Geometry Name="Test" Model="Test"><Geometry Name="Second level" Model="Yolo"/></Geometry></Geometries></FixtureType></GDTF>"#
            )
        )
    }
}
