use strum::EnumString;

#[derive(Debug, EnumString, PartialEq, Default, strum::Display)]
pub enum DataVersion {
    #[strum(to_string = "1.1")]
    V1_1,
    #[strum(to_string = "1.2")]
    V1_2,
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use crate::{utils::GetAttribute, Problem};

    use super::*;

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
