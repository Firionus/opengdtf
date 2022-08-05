use std::any::type_name;
use std::str::FromStr;

use crate::errors::*;
use roxmltree::Document;

use roxmltree::Node;

use crate::node_position;
use crate::Problem;

pub(crate) fn maybe_get_string_attribute(
    nopt: &Option<Node>,
    attr: &str,
    problems: &mut Vec<Problem>,
    doc: &Document,
) -> String {
    nopt.and_then(|n| n.get_attribute(attr, problems, doc))
        .unwrap_or_else(|| "".to_owned())
}

pub(crate) trait GetAttribute {
    fn get_attribute<T: FromStr>(&self, attr: &str, problems: &mut Vec<Problem>, doc: &Document) -> Option<T> where
    <T as FromStr>::Err: std::error::Error + 'static;
}

impl GetAttribute for Node<'_, '_> {
    fn get_attribute<T: FromStr>(
        &self,
        attr: &str,
        problems: &mut Vec<Problem>,
        doc: &Document,
    ) -> Option<T> where
    <T as FromStr>::Err: std::error::Error + 'static {
        let content = self.attribute(attr)
        .or_else(|| {
            problems.push_then_none(Problem::XmlAttributeMissing {
                attr: attr.to_owned(),
                tag: self.tag_name().name().to_owned(),
                pos: node_position(self, doc),
            })
        })?;
        match content.parse::<T>() {
            Ok(v) => Some(v),
            Err(err) => problems.push_then_none(Problem::InvalidAttribute {
                attr: attr.to_owned(),
                tag: self.tag_name().name().to_owned(),
                pos: node_position(self, doc),
                content: content.to_owned(),
                err: Box::new(err),
                expected_type: type_name::<T>().to_owned(),
            }),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_attribute_works() {
        let xml = r#"<tag attr="300" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        assert_eq!(n.get_attribute("attr", &mut problems, &doc), Some(300u32));
        assert_eq!(n.get_attribute::<u8>("attr", &mut problems, &doc), None);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn get_attribute_errors() {
        let xml = r#"<Geometry pos="not_a_number" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        assert_eq!(n.get_attribute("pos", &mut problems, &doc), Some("not_a_number".to_string()));
        assert_eq!(n.get_attribute::<i16>("pos", &mut problems, &doc), None);
        assert_eq!(problems.len(), 1);
        println!("{}", problems[0]);
    }
}
