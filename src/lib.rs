mod errors;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use errors::GdtfCompleteFailure;

#[derive(Debug)]
pub struct Gdtf {
    pub data_version: String,
    pub errors: Option<()>, // TODO lay out type
}

// TODO when does TryFrom fail and when is it succesful with an error list?
// When there's full failure (i.e. it's not XML, tree parsing error, no root node, ...), return an Err
// When anything can be gained from it, return (data, errors)

// TODO implement the error list

// TODO replace unwraps by non-panicking code

impl TryFrom<&str> for Gdtf {
    type Error = GdtfCompleteFailure;

    fn try_from(description_content: &str) -> Result<Self, Self::Error> {
        let doc = roxmltree::Document::parse(&description_content)?;

        let gdtf = Gdtf {
            data_version: doc
                .descendants()
                .find(|n| n.has_tag_name("GDTF"))
                .unwrap()
                .attribute("DataVersion")
                .unwrap()
                .into(), // TODO validate DataVersion format
            errors: None,
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
        let reader = File::open(path).unwrap();
        let mut zip = zip::ZipArchive::new(reader).unwrap();
        let mut file = zip.by_name("description.xml").unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        Ok(Gdtf::try_from(&content[..]).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{errors::GdtfCompleteFailure, Gdtf};

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
        assert!(msg == "Invalid XML: expected 'this' tag, not 'that' at 1:7");
    }
}
