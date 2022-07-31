mod errors;
pub use errors::*;
mod parts;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use indextree::Arena;
use parts::gdtf_node::*;
use parts::geometries::*;
use roxmltree::Node;
use uuid::Uuid;

#[derive(Debug)]
pub struct Gdtf {
    // File Information
    pub data_version: String,
    pub fixture_type_id: Uuid,
    pub ref_ft: Option<Uuid>,
    pub can_have_children: bool,
    // Metadata
    pub name: String,
    pub short_name: String,
    pub long_name: String,
    pub manufacturer: String,
    pub description: String,

    pub geometries: Arena<GeometryType>,

    // Library Related
    pub problems: Vec<Problem>,
}

impl TryFrom<&str> for Gdtf {
    type Error = Error;

    fn try_from(description_content: &str) -> Result<Self, Self::Error> {
        let doc = roxmltree::Document::parse(description_content)?;

        let mut problems: Vec<Problem> = vec![];

        let (root_node, data_version) = parse_gdtf_node(&doc, &mut problems)?;

        root_node.descendants().for_each(|n| println!("{:#?}", n));

        let ft = root_node
            .descendants()
            .find(|n| n.has_tag_name("FixtureType"))
            .or_else(|| {
                problems.addn(Problem::NodeMissing {
                    missing: "FixtureType".to_owned(),
                    parent: "GDTF".to_owned(),
                })
            });

        let mut geometries = Arena::new();

        match ft {
            Some(ft) => parse_geometries(&mut geometries, &ft, &mut problems),
            None => (),
        }

        let gdtf = Gdtf {
            data_version,
            // TODO all of this would be one level less nested if ft could be unwrapped - how to architect that?
            fixture_type_id: ft
                .and_then(|n| {
                    n.attribute("FixtureTypeID").or_else(|| {
                        problems.addn(Problem::AttributeMissing {
                            attr: "FixtureTypeId".to_owned(),
                            node: "FixtureType".to_owned(),
                        })
                    })
                })
                .and_then(|s| match Uuid::try_from(s) {
                    Ok(v) => Some(v),
                    Err(e) => problems.addn(Problem::UuidError(e, "FixtureTypeId".to_owned())),
                })
                .unwrap_or(Uuid::nil()),
            ref_ft: ft
                .and_then(|n| n.attribute("RefFT")) // I think it's okay to not have this
                .and_then(|s| match s {
                    "" => None,
                    _ => match Uuid::try_from(s) {
                        Ok(v) => Some(v),
                        Err(e) => problems.addn(Problem::UuidError(e, "RefFT".to_owned())),
                    },
                }),
            can_have_children: ft
                .and_then(|n| n.attribute("CanHaveChildren"))
                .and_then(|s| match s {
                    "Yes" => Some(true),
                    "No" => Some(false),
                    _ => problems.addn(Problem::InvalidYesNoEnum(
                        s.to_owned(),
                        "CanHaveChildren".to_owned(),
                    )),
                })
                .unwrap_or(true),
            name: get_string_attribute(&ft, "Name", &mut problems),
            short_name: get_string_attribute(&ft, "ShortName", &mut problems),
            long_name: get_string_attribute(&ft, "LongName", &mut problems),
            description: get_string_attribute(&ft, "Description", &mut problems),
            manufacturer: get_string_attribute(&ft, "Manufacturer", &mut problems),
            geometries,
            problems,
        };

        Ok(gdtf)
    }
}

trait ProblemAdd {
    /// Push a Problem and Return None
    fn addn<T>(&mut self, e: Problem) -> Option<T>;
}

impl ProblemAdd for Vec<Problem> {
    fn addn<T>(&mut self, e: Problem) -> Option<T> {
        self.push(e);
        None
    }
}

fn get_string_attribute(nopt: &Option<Node>, attr: &str, problems: &mut Vec<Problem>) -> String {
    nopt.and_then(|n| {
        n.attribute(attr).or_else(|| {
            problems.addn(Problem::AttributeMissing {
                attr: attr.to_owned(),
                node: n.tag_name().name().to_owned(), // TODO this might not be very useful if the node name occurs often
            })
        })
    })
    .unwrap_or("")
    .to_owned()
}

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

    use crate::{errors::Error, Gdtf};

    #[test]
    fn data_version_parsing() {
        let path = Path::new(
            "test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.gdtf",
        );
        let gdtf = Gdtf::try_from(path).unwrap();
        assert_eq!(gdtf.data_version, "1.1");
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
