use roxmltree::{Document, Node};

use crate::{node_position, Error, Problem};

pub fn parse_gdtf_node<'a>(
    doc: &'a Document,
    problems: &mut Vec<Problem>,
) -> Result<(Node<'a, 'a>, String), Error> {
    let root_node = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(Error::NoRootNode)?;

    let data_version = root_node
        .attribute("DataVersion")
        .map(|s| {
            // validate
            match s {
                "1.1" | "1.2" => (),
                _ => problems.push(Problem::InvalidDataVersion(
                    s.to_owned(),
                    node_position(&root_node, doc),
                )),
            };
            s
        })
        .unwrap_or_else(|| {
            // handle missing
            problems.push(Problem::NoDataVersion(node_position(&root_node, doc)));
            ""
        })
        .into();

    Ok((root_node, data_version))
}

#[cfg(test)]
mod tests {
    use crate::{errors::Problem, parts::gdtf_node};

    #[test]
    fn data_version_missing() {
        let invalid_xml = "<GDTF></GDTF>";
        let doc = roxmltree::Document::parse(invalid_xml).unwrap();
        let mut problems: Vec<Problem> = vec![];

        let (_root_node, data_version) = gdtf_node::parse_gdtf_node(&doc, &mut problems).unwrap();

        assert!(data_version.is_empty()); // Default value (empty string) should be applied
        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::NoDataVersion(..)));

        let msg = format!("{}", &problems[0]);
        assert!(msg.contains("missing attribute 'DataVersion' on 'GDTF' node"));
    }

    #[test]
    fn data_version_invalid_value() {
        let invalid_xml = r#"<GDTF DataVersion="1.0"></GDTF>"#;
        let doc = roxmltree::Document::parse(invalid_xml).unwrap();
        let mut problems: Vec<Problem> = vec![];

        let (_root_node, data_version) = gdtf_node::parse_gdtf_node(&doc, &mut problems).unwrap();

        assert!(&data_version == "1.0"); // Wrong value should be output

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::InvalidDataVersion(..)));
        let msg = format!("{}", &problems[0]);
        assert!(msg.contains("attribute 'DataVersion' of 'GDTF' node"));
    }
}
