use roxmltree::{Document, Node};
use strum::EnumString;

use crate::{Error, Problem, utils::GetAttribute};

#[derive(Debug, EnumString, PartialEq, Default)]
pub enum DataVersion {
    #[strum(serialize="1.1")]
    V1_1,
    #[strum(serialize="1.2")]
    V1_2,
    #[default]
    Unknown,
}

pub fn parse_gdtf_node<'a>(
    doc: &'a Document,
    problems: &mut Vec<Problem>,
) -> Result<(Node<'a, 'a>, DataVersion), Error> {
    let root_node = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(Error::NoRootNode)?;

    let data_version = root_node.get_attribute("DataVersion", problems, doc).unwrap_or(DataVersion::Unknown);

    Ok((root_node, data_version))
}

#[cfg(test)]
mod tests {
    use crate::utils::GetAttribute;

    use super::*;

    #[test]
    fn data_version_missing() {
        let invalid_xml = "<GDTF></GDTF>";
        let doc = roxmltree::Document::parse(invalid_xml).unwrap();
        let mut problems: Vec<Problem> = vec![];

        let (_root_node, data_version) = parse_gdtf_node(&doc, &mut problems).unwrap();

        assert_eq!(data_version, DataVersion::Unknown); // Default value (empty string) should be applied
        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::XmlAttributeMissing { .. }));

        let msg = format!("{}", &problems[0]);
        assert!(msg.contains("attribute 'DataVersion' missing on 'GDTF'"));
    }

    #[test]
    fn data_version_invalid_value() {
        let invalid_xml = r#"<GDTF DataVersion="1.0"></GDTF>"#;
        let doc = roxmltree::Document::parse(invalid_xml).unwrap();
        let mut problems: Vec<Problem> = vec![];

        let (_root_node, data_version) = parse_gdtf_node(&doc, &mut problems).unwrap();

        assert_eq!(data_version, DataVersion::Unknown); // Wrong value should be output

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::InvalidAttribute{ .. }));
        let msg = format!("{}", &problems[0]);
        assert!(msg.contains("attribute 'DataVersion' on 'GDTF'"));
    }

    #[test]
    fn data_version_parsing_with_get_attribute() {
        let xml = r#"<GDTF DataVersion="1.0"></GDTF>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut problems: Vec<Problem> = vec![];
        let root_node = doc.root_element();

        let dv: Option<DataVersion> = root_node.get_attribute("DataVersion", &mut problems, &doc);
        assert_eq!(problems.len(), 1);
        assert_eq!(dv, None);

        let xml = r#"<GDTF DataVersion="1.1"></GDTF>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut problems: Vec<Problem> = vec![];
        let root_node = doc.root_element();

        let dv: Option<DataVersion> = root_node.get_attribute("DataVersion", &mut problems, &doc);
        assert_eq!(problems.len(), 0);
        assert_eq!(dv, Some(DataVersion::V1_1));
    }
}
