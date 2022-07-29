pub mod errors;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use errors::{GdtfCompleteFailure, GdtfProblem};

#[derive(Debug)]
pub struct Gdtf {
    pub data_version: String,
    pub problems: Vec<GdtfProblem>,
}

impl TryFrom<&str> for Gdtf {
    type Error = GdtfCompleteFailure;

    fn try_from(description_content: &str) -> Result<Self, Self::Error> {
        let doc = roxmltree::Document::parse(description_content)?;

        let root_node = doc
            .descendants()
            .find(|n| n.has_tag_name("GDTF"))
            .ok_or(GdtfCompleteFailure::NoRootNode)?;

        let mut problems: Vec<GdtfProblem> = vec![];

        let data_version = root_node
            .attribute("DataVersion")
            .map(|s| {
                // validate
                match s {
                    "1.1" => (),
                    "1.2" => (),
                    _ => problems.push(GdtfProblem::InvalidDataVersion(s.to_owned())),
                };
                s
            })
            .unwrap_or_else(|| {
                // handle missing
                problems.push(GdtfProblem::NoDataVersion);
                ""
            })
            .into();

        let gdtf = Gdtf {
            data_version,
            problems,
        };

        Ok(gdtf)
    }
}

impl TryFrom<&String> for Gdtf {
    type Error = GdtfCompleteFailure;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Gdtf::try_from(&value[..])
    }
}

impl TryFrom<&Path> for Gdtf {
    type Error = GdtfCompleteFailure;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let zipfile =
            File::open(path).map_err(|e| GdtfCompleteFailure::OpenError(path.into(), e))?;
        let mut zip = zip::ZipArchive::new(zipfile)?;
        let mut file = zip
            .by_name("description.xml")
            .map_err(GdtfCompleteFailure::DescriptionXmlMissing)?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(GdtfCompleteFailure::DescriptionXmlReadError)?;

        Gdtf::try_from(&content[..])
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{errors::GdtfCompleteFailure, errors::GdtfProblem, Gdtf};

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
        assert!(matches!(&e, GdtfCompleteFailure::XmlError(..)));
        let msg: String = format!("{}", e);
        assert!(msg == "invalid XML: expected 'this' tag, not 'that' at 1:7");
    }

    #[test]
    fn no_root_node_error() {
        let invalid_xml = "<this></this>";
        let res = Gdtf::try_from(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, GdtfCompleteFailure::NoRootNode));
    }

    #[test]
    fn data_version_missing() {
        let invalid_xml = "<GDTF></GDTF>";
        let gdtf = Gdtf::try_from(invalid_xml).unwrap();
        assert!(&gdtf.data_version.is_empty()); // Default value should be applied
        assert!(gdtf.problems == vec![GdtfProblem::NoDataVersion]);
        let msg = format!("{}", &gdtf.problems[0]);
        assert!(msg == "missing attribute 'DataVersion' on 'GDTF' node");
    }

    #[test]
    fn data_version_invalid_format() {
        let invalid_xml = r#"<GDTF DataVersion="1.0"></GDTF>"#;
        let gdtf = Gdtf::try_from(invalid_xml).unwrap();
        assert!(&gdtf.data_version == "1.0"); // Wrong value should be output
        assert!(gdtf.problems == vec![GdtfProblem::InvalidDataVersion("1.0".to_owned())]);
        let msg = format!("{}", &gdtf.problems[0]);
        assert!(msg == "attribute 'DataVersion' of 'GDTF' node is invalid. Got '1.0'.");
    }

    #[test]
    fn file_not_found() {
        let path = Path::new("this/does/not/exist");
        let e = Gdtf::try_from(path).unwrap_err();
        assert!(matches!(e, GdtfCompleteFailure::OpenError(..)));
    }
}
