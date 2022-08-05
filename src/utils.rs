use std::any::type_name;
use std::str::FromStr;

use crate::errors::*;
use roxmltree::Document;

use roxmltree::Node;

use crate::node_position;
use crate::Problem;

// TODO change to method on Node?
// TODO integrate into get_attribute?
pub(crate) fn get_string_attribute(
    n: &Node,
    attr: &str,
    problems: &mut Vec<Problem>,
    doc: &Document,
) -> Option<String> {
    n.attribute(attr)
        .or_else(|| {
            problems.push_then_none(Problem::XmlAttributeMissing {
                attr: attr.to_owned(),
                tag: n.tag_name().name().to_owned(),
                pos: node_position(n, doc),
            })
        })
        .map(|s| s.to_owned())
}

pub(crate) fn maybe_get_string_attribute(
    nopt: &Option<Node>,
    attr: &str,
    problems: &mut Vec<Problem>,
    doc: &Document,
) -> String {
    nopt.and_then(|n| get_string_attribute(&n, attr, problems, doc))
        .unwrap_or_else(|| "".to_owned())
}

// pub(crate) fn get_u32_attribute(
//     n: &Node,
//     attr: &str,
//     problems: &mut Vec<Problem>,
//     doc: &Document,
// ) -> Option<u32> {
//     match get_string_attribute(n, attr, problems, doc)?.parse() {
//         Ok(v) => Some(v),
//         Err(err) => problems.push_then_none(Problem::InvalidInteger {
//             attr: attr.to_owned(),
//             tag: n.tag_name().name().to_owned(),
//             pos: node_position(n, doc),
//             err,
//         }),
//     }
// }

pub(crate) fn get_attribute<T: FromStr>(
    n: &Node,
    attr: &str,
    problems: &mut Vec<Problem>,
    doc: &Document,
) -> Option<T> where
<T as FromStr>::Err: std::error::Error + 'static {
    let content = get_string_attribute(n, attr, problems, doc)?;
    match content.parse::<T>() {
        Ok(v) => Some(v),
        Err(err) => problems.push_then_none(Problem::InvalidAttribute {
            attr: attr.to_owned(),
            tag: n.tag_name().name().to_owned(),
            pos: node_position(n, doc),
            content,
            err: Box::new(err),
            expected_type: type_name::<T>().to_owned(),
        }),
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
        assert_eq!(get_attribute(&n, "attr", &mut problems, &doc), Some(300u32));
        assert_eq!(get_attribute::<u8>(&n, "attr", &mut problems, &doc), None);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn get_attribute_errors() {
        let xml = r#"<Geometry pos="not_a_number" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        assert_eq!(get_attribute(&n, "pos", &mut problems, &doc), Some("not_a_number".to_string()));
        assert_eq!(get_attribute::<i16>(&n, "pos", &mut problems, &doc), None);
        assert_eq!(problems.len(), 1);
        println!("{}", problems[0]);
    }
}
