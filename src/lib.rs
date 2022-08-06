mod errors;
pub use errors::*;
use utils::GetAttribute;
mod parts;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use parts::data_version::*;
use parts::dmx_modes::*;
use parts::geometries::*;
use uuid::Uuid;

#[derive(Debug)]
pub struct Gdtf {
    // File Information
    pub data_version: DataVersion,
    pub fixture_type_id: Uuid,
    pub ref_ft: Option<Uuid>,
    pub can_have_children: bool,

    // Metadata
    pub name: String,
    pub short_name: String,
    pub long_name: String,
    pub manufacturer: String,
    pub description: String,

    pub geometries: Geometries,

    pub dmx_modes: Vec<DmxMode>,

    // Parsing
    pub problems: Vec<Problem>,
}

impl Default for Gdtf {
    fn default() -> Self {
        Self {
            data_version: Default::default(),
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
            problems: Default::default(),
        }
    }
}

impl TryFrom<&str> for Gdtf {
    type Error = Error;

    fn try_from(description_content: &str) -> Result<Self, Self::Error> {
        let doc = roxmltree::Document::parse(description_content)?;

        let mut gdtf = Gdtf::default();

        let problems = &mut gdtf.problems;

        let root_node = doc
            .descendants()
            .find(|n| n.has_tag_name("GDTF"))
            .ok_or(Error::NoRootNode)?;

        if let Some(val) = root_node.parse_required_attribute("DataVersion", problems, &doc) {
            gdtf.data_version = val;
        };

        let ft = root_node
            .children()
            .find(|n| n.has_tag_name("FixtureType"))
            .or_else(|| {
                problems.push_then_none(Problem::XmlNodeMissing {
                    missing: "FixtureType".to_owned(),
                    parent: "GDTF".to_owned(),
                    pos: node_position(&root_node, &doc),
                })
            });

        let geometries = &mut gdtf.geometries;

        if let Some(ft) = ft {
            parse_geometries(geometries, &ft, problems, &doc);

            gdtf.fixture_type_id = ft
                .attribute("FixtureTypeID")
                .or_else(|| {
                    problems.push_then_none(Problem::XmlAttributeMissing {
                        attr: "FixtureTypeId".to_owned(),
                        tag: "FixtureType".to_owned(),
                        pos: node_position(&ft, &doc),
                    })
                })
                .and_then(|s| match Uuid::try_from(s) {
                    Ok(v) => Some(v),
                    Err(e) => problems.push_then_none(Problem::UuidError(
                        e,
                        "FixtureTypeId".to_owned(),
                        node_position(&ft, &doc),
                    )),
                })
                .unwrap_or(Uuid::nil());

            gdtf.ref_ft = ft
                .attribute("RefFT")
                // no handling if missing, I don't think it's important to have the node present when the value is empty
                .and_then(|s| match s {
                    "" => None,
                    _ => match Uuid::try_from(s) {
                        Ok(v) => Some(v),
                        Err(e) => problems.push_then_none(Problem::UuidError(
                            e,
                            "RefFT".to_owned(),
                            node_position(&ft, &doc),
                        )),
                    },
                });

            if let Some(can_have_children) = ft.attribute("CanHaveChildren").and_then(|s| match s {
                "Yes" => Some(true),
                "No" => Some(false),
                _ => problems.push_then_none(Problem::InvalidYesNoEnum(
                    s.to_owned(),
                    "CanHaveChildren".to_owned(),
                    node_position(&ft, &doc),
                )),
            }) {
                gdtf.can_have_children = can_have_children;
            };

            if let Some(val) = ft.parse_required_attribute("Name", problems, &doc) {
                gdtf.name = val;
            };

            if let Some(val) = ft.parse_required_attribute("ShortName", problems, &doc) {
                gdtf.short_name = val;
            };

            if let Some(val) = ft.parse_required_attribute("LongName", problems, &doc) {
                gdtf.long_name = val;
            };

            if let Some(val) = ft.parse_required_attribute("Description", problems, &doc) {
                gdtf.description = val;
            };

            if let Some(val) = ft.parse_required_attribute("Manufacturer", problems, &doc) {
                gdtf.manufacturer = val;
            };
        }

        Ok(gdtf)
    }
}

mod utils;

impl TryFrom<&String> for Gdtf {
    type Error = Error;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Gdtf::try_from(&value[..])
    }
}

impl TryFrom<&Path> for Gdtf {
    type Error = Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let zipfile = File::open(path).map_err(|e| Error::OpenError(path.into(), e))?;
        let mut zip = zip::ZipArchive::new(zipfile)?;
        let mut file = zip
            .by_name("description.xml")
            .map_err(Error::DescriptionXmlMissing)?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(Error::DescriptionXmlReadError)?;

        Gdtf::try_from(&content[..])
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn channel_layout_test() {
        let path = Path::new(
            "test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.gdtf",
        );
        let gdtf = Gdtf::try_from(path).unwrap();
        assert_eq!(gdtf.data_version, DataVersion::V1_1);
        assert!(gdtf.problems.is_empty());
    }

    #[test]
    fn robe_tetra2_slightly_broken() {
        let path = Path::new(
            "test/resources/Robe_Lighting@Robin_Tetra2@04062021.gdtf",
        );
        let gdtf = Gdtf::try_from(path).unwrap();
        assert_eq!(gdtf.data_version, DataVersion::V1_1);
        // Problems with duplicate Geometry Names
        assert_eq!(gdtf.problems.len(), 18);
        gdtf.problems.iter().for_each(|prob| {
            assert!(matches!(prob, Problem::DuplicateGeometryName( .. )))
        });
        // TODO assert all channels properly find their geometries even with
        // duplicate geometry names
    }

    #[test]
    fn xml_error() {
        let invalid_xml = "<this></that>";
        let res = Gdtf::try_from(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, Error::XmlError(..)));
        let msg: String = format!("{}", e);
        assert!(msg == "invalid XML: expected 'this' tag, not 'that' at 1:7");
    }

    #[test]
    fn no_root_node_error() {
        let invalid_xml = "<this></this>";
        let res = Gdtf::try_from(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, Error::NoRootNode));
    }

    #[test]
    fn file_not_found() {
        let path = Path::new("this/does/not/exist");
        let e = Gdtf::try_from(path).unwrap_err();
        assert!(matches!(e, Error::OpenError(..)));
    }

    #[test]
    fn description_xml_missing() {
        let path = Path::new(
            "test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.empty.gdtf",
        );
        let e = Gdtf::try_from(path).unwrap_err();
        assert!(matches!(e, Error::DescriptionXmlMissing(..)));
    }
}
