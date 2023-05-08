use std::any::type_name;

use std::str::FromStr;

use roxmltree::Node;

use crate::{parser::problems::HandleProblem, types::name::Name, Problem, ProblemAt, Problems};

pub(crate) trait GetXmlAttribute {
    fn required_attribute(&self, attr: &str) -> Result<&str, ProblemAt>;

    fn parse_required_attribute<T: FromStr>(&self, attr: &str) -> Result<T, ProblemAt>
    where
        <T as FromStr>::Err: std::error::Error + 'static;

    fn parse_required_attribute_or<T: FromStr>(
        &self,
        attr: &str,
        default: T,
        problems: &mut Problems,
    ) -> T
    where
        <T as FromStr>::Err: std::error::Error + 'static;

    fn parse_required_attribute_or_default<T: FromStr + Default>(
        &self,
        attr: &str,
        problems: &mut Problems,
    ) -> T
    where
        <T as FromStr>::Err: std::error::Error + 'static;

    fn parse_attribute<T: FromStr>(&self, attr: &str) -> Option<Result<T, ProblemAt>>
    where
        <T as FromStr>::Err: std::error::Error + 'static;

    fn map_parse_attribute<T: FromStr, F>(&self, attr: &str, f: F) -> Option<Result<T, ProblemAt>>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
        F: FnOnce(Option<&str>) -> Option<&str>;

    fn name(&self, node_index_in_xml_parent: usize, problems: &mut Problems) -> Name;
}

impl GetXmlAttribute for Node<'_, '_> {
    /// Returns value of an atrribute, or a problem if missing.
    fn required_attribute(&self, name: &str) -> Result<&str, ProblemAt> {
        self.attribute(name).ok_or_else(|| {
            Problem::XmlAttributeMissing {
                attr: name.to_owned(),
                tag: self.tag_name().name().to_owned(),
            }
            .at(self)
        })
    }

    /// Parse an XML attribute to the type `T`.
    ///
    /// If the attribute is missing or it can't be parsed to `T`, a problem is
    /// returned.
    fn parse_required_attribute<T: FromStr>(&self, attr: &str) -> Result<T, ProblemAt>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
    {
        let content = self.required_attribute(attr)?;
        parse_attribute_content(self, content, attr)
    }

    fn parse_required_attribute_or<T: FromStr>(
        &self,
        attr: &str,
        opt: T,
        problems: &mut Problems,
    ) -> T
    where
        <T as FromStr>::Err: std::error::Error + 'static,
    {
        self.parse_required_attribute(attr)
            .ok_or_handled_by("using default", problems)
            .unwrap_or(opt)
    }

    fn parse_required_attribute_or_default<T: FromStr + Default>(
        &self,
        attr: &str,
        problems: &mut Problems,
    ) -> T
    where
        <T as FromStr>::Err: std::error::Error + 'static,
    {
        self.parse_required_attribute(attr)
            .ok_or_handled_by("using default", problems)
            .unwrap_or_default()
    }

    /// Parse an optional XML attribute to the type `T`. If it is missing,
    /// returns None.
    fn parse_attribute<T: FromStr>(&self, attr: &str) -> Option<Result<T, ProblemAt>>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
    {
        let content = self.attribute(attr)?;
        Some(parse_attribute_content(self, content, attr))
    }

    /// Get an optional XML attribute, apply the closure and parse it to `T`.
    ///
    /// If the closure returns None, the function returns None; otherwise
    /// continues with parsing. This allows avoiding parsing under certain
    /// conditions.
    fn map_parse_attribute<T: FromStr, F>(&self, attr: &str, f: F) -> Option<Result<T, ProblemAt>>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
        F: FnOnce(Option<&str>) -> Option<&str>,
    {
        let content = f(self.attribute(attr))?;
        Some(parse_attribute_content(self, content, attr))
    }

    /// Get attribute "Name" and parse to GDTF type Name.
    ///
    /// node_index_in_xml_parent is a 0-based index.
    ///
    /// If missing, provide a default and push a problem. If the Name is
    /// invalid, replace the disallowed chars and push a problem.
    fn name(&self, node_index_in_xml_parent: usize, problems: &mut Problems) -> Name {
        self.required_attribute("Name")
            .map(|name| parse_name_or_fix(self, name, problems))
            .unwrap_or_else(|p| {
                let default_name =
                    Name::valid_default(self.tag_name().name(), node_index_in_xml_parent);
                p.handled_by(format!("using default name '{default_name}'"), problems);
                default_name
            })
    }
}

pub(crate) fn parse_attribute_content<T: FromStr>(
    node: &Node,
    content: &str,
    attr: &str,
) -> Result<T, ProblemAt>
where
    <T as FromStr>::Err: std::error::Error + 'static,
{
    content.parse::<T>().map_err(|err| {
        Problem::InvalidAttribute {
            attr: attr.to_owned(),
            tag: node.tag_name().name().to_owned(),
            content: content.to_owned(),
            source: Box::new(err),
            expected_type: type_name::<T>().to_owned(),
        }
        .at(node)
    })
}

fn parse_name_or_fix(node: &Node, name: &str, problems: &mut Vec<crate::HandledProblem>) -> Name {
    Name::try_from(name).unwrap_or_else(|e| {
        let fixed = e.fixed.clone();
        Problem::InvalidAttribute {
            attr: "Name".into(),
            tag: node.tag_name().name().to_owned(),
            content: name.to_owned(),
            source: Box::new(e),
            expected_type: "Name".to_owned(),
        }
        .at(node)
        .handled_by("replacing invalid chars with '□'", problems);
        fixed
    })
}

#[cfg(test)]
mod tests {
    use crate::parser::problems::HandleProblem;

    use super::*;

    #[test]
    fn test_parse_required_attribute() {
        let xml = r#"<tag attr="300" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Problems = vec![];
        assert_eq!(
            n.parse_required_attribute::<u32>("attr")
                .ok_or_handled_by("setting None", &mut problems),
            Some(300)
        );
        assert_eq!(
            n.parse_required_attribute::<String>("attr")
                .ok_or_handled_by("setting None", &mut problems),
            Some("300".to_string())
        );
        assert_eq!(
            n.parse_required_attribute::<u8>("attr")
                .ok_or_handled_by("setting None", &mut problems),
            None
        );
        assert_eq!(
            n.parse_required_attribute::<String>("missing")
                .ok_or_handled_by("setting None", &mut problems),
            None
        );
        assert_eq!(problems.len(), 2);
        let mut problems = problems.iter();
        assert!(matches!(
            problems.next().unwrap().problem(),
            Problem::InvalidAttribute {
                        attr,
                        tag,
                        content,
                        expected_type,
                        ..
                }
        if attr == "attr" && tag == "tag" && content == "300" && expected_type == "u8"));
        assert!(matches!(
            problems.next().unwrap().problem(),
            Problem::XmlAttributeMissing { attr, tag }
        if attr == "missing" && tag == "tag"));
    }
}
