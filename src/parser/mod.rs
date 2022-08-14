mod errors;
mod utils;
mod geometries;

use std::{io::{Read, Seek}};

use uuid::Uuid;

use crate::Gdtf;

use self::{utils::{GetAttribute, XmlPosition}, geometries::parse_geometries, errors::{Error, Problem, ProblemAdd}};

#[derive(Debug)]
pub struct ParsedGdtf {
    pub gdtf: Gdtf,
    pub problems: Vec<Problem>,
}

// TODO should probably read from Buffer instead of Path, if the file is in RAM
// also is better to separate concerns, this is not a path opening library
pub fn parse<T: Read + Seek>(reader: T) -> Result<ParsedGdtf, Error> {
    // TODO remove line
    // let zipfile = File::open(path).map_err(|e| Error::OpenError(path.into(), e))?;
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut file = zip
        .by_name("description.xml")
        .map_err(Error::DescriptionXmlMissing)?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(Error::InvalidDescriptionXml)?;

    parse_description(&content[..])
}

fn parse_description(description_content: &str) -> Result<ParsedGdtf, Error> {
    let doc = roxmltree::Document::parse(description_content)?;

        let mut gdtf = Gdtf::default();

        let mut problems = vec![];

        let root_node = doc
            .descendants()
            .find(|n| n.has_tag_name("GDTF"))
            .ok_or(Error::NoRootNode)?;

        if let Some(val) = root_node.parse_required_attribute("DataVersion", &mut problems, &doc) {
            gdtf.data_version = val;
        };

        let ft = root_node
            .children()
            .find(|n| n.has_tag_name("FixtureType"))
            .or_else(|| {
                problems.push_then_none(Problem::XmlNodeMissing {
                    missing: "FixtureType".to_owned(),
                    parent: "GDTF".to_owned(),
                    pos: root_node.position(&doc),
                })
            });

        let geometries = &mut gdtf.geometries;

        if let Some(ft) = ft {
            parse_geometries(geometries, &ft, &mut problems, &doc);

            gdtf.fixture_type_id = ft
                .attribute("FixtureTypeID")
                .or_else(|| {
                    problems.push_then_none(Problem::XmlAttributeMissing {
                        attr: "FixtureTypeId".to_owned(),
                        tag: "FixtureType".to_owned(),
                        pos: ft.position(&doc),
                    })
                })
                .and_then(|s| match Uuid::try_from(s) {
                    Ok(v) => Some(v),
                    Err(e) => problems.push_then_none(Problem::UuidError(
                        e,
                        "FixtureTypeId".to_owned(),
                        ft.position(&doc),
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
                            ft.position(&doc),
                        )),
                    },
                });

            if let Some(can_have_children) = ft.attribute("CanHaveChildren").and_then(|s| match s {
                "Yes" => Some(true),
                "No" => Some(false),
                _ => problems.push_then_none(Problem::InvalidYesNoEnum(
                    s.to_owned(),
                    "CanHaveChildren".to_owned(),
                    ft.position(&doc),
                )),
            }) {
                gdtf.can_have_children = can_have_children;
            };

            if let Some(val) = ft.parse_required_attribute("Name", &mut problems, &doc) {
                gdtf.name = val;
            };

            if let Some(val) = ft.parse_required_attribute("ShortName", &mut problems, &doc) {
                gdtf.short_name = val;
            };

            if let Some(val) = ft.parse_required_attribute("LongName", &mut problems, &doc) {
                gdtf.long_name = val;
            };

            if let Some(val) = ft.parse_required_attribute("Description", &mut problems, &doc) {
                gdtf.description = val;
            };

            if let Some(val) = ft.parse_required_attribute("Manufacturer", &mut problems, &doc) {
                gdtf.manufacturer = val;
            };
        }

        Ok(ParsedGdtf{gdtf, problems})
}

#[cfg(test)]
mod tests {
    use std::{path::Path, fs::File};

    use crate::DataVersion;

    use super::*;

    #[test]
    fn channel_layout_test() {
        let path = Path::new(
            "test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.gdtf",
        );
        let file = File::open(path).unwrap();
        let ParsedGdtf {gdtf, problems} = parse(file).unwrap();
        assert_eq!(gdtf.data_version, DataVersion::V1_1);
        assert!(problems.is_empty());
    }

    #[test]
    fn robe_tetra2_slightly_broken() {
        let path = Path::new(
            "test/resources/Robe_Lighting@Robin_Tetra2@04062021.gdtf",
        );
        let file = File::open(path).unwrap();
        let ParsedGdtf {gdtf, problems} = parse(file).unwrap();
        assert_eq!(gdtf.data_version, DataVersion::V1_1);
        // Problems with duplicate Geometry Names
        assert_eq!(problems.len(), 18);
        problems.iter().for_each(|prob| {
            assert!(matches!(prob, Problem::DuplicateGeometryName( .. )))
        });
        // TODO assert all channels properly find their geometries even with
        // duplicate geometry names
    }

    #[test]
    fn xml_error() {
        let invalid_xml = "<this></that>";
        let res = parse_description(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, Error::InvalidXml(..)));
        let msg: String = format!("{}", e);
        assert!(msg == "invalid XML: expected 'this' tag, not 'that' at 1:7");
    }

    #[test]
    fn no_root_node_error() {
        let invalid_xml = "<this></this>";
        let res = parse_description(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, Error::NoRootNode));
    }

    #[test]
    fn description_xml_missing() {
        let path = Path::new(
            "test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.empty.gdtf",
        );
        let file = File::open(path).unwrap();
        let e = parse(file).unwrap_err();
        assert!(matches!(e, Error::DescriptionXmlMissing(..)));
    }

    #[test]
    fn data_version_parsing_with_get_attribute() {
        let xml = r#"<GDTF DataVersion="1.0"></GDTF>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut problems: Vec<Problem> = vec![];
        let root_node = doc.root_element();

        let dv: Option<DataVersion> =
            root_node.parse_required_attribute("DataVersion", &mut problems, &doc);
        assert_eq!(problems.len(), 1);
        assert_eq!(dv, None);

        let xml = r#"<GDTF DataVersion="1.1"></GDTF>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut problems: Vec<Problem> = vec![];
        let root_node = doc.root_element();

        let dv: Option<DataVersion> =
            root_node.parse_required_attribute("DataVersion", &mut problems, &doc);
        assert_eq!(problems.len(), 0);
        assert_eq!(dv, Some(DataVersion::V1_1));
        assert_eq!(format!("{}", dv.unwrap()), "1.1");
    }
}