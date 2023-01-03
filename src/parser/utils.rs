use std::any::type_name;
use std::fmt::Display;
use std::str::FromStr;

use roxmltree::Node;
use roxmltree::TextPos;

use super::errors::*;

// TODO is name accurate?
pub(crate) trait GetFromNode {
    fn parse_required_attribute<T: FromStr>(&self, attr: &str) -> Result<T, Problem>
    where
        <T as FromStr>::Err: std::error::Error + 'static;

    fn parse_attribute<T: FromStr>(&self, attr: &str) -> Option<Result<T, Problem>>
    where
        <T as FromStr>::Err: std::error::Error + 'static;

    fn required_attribute(&self, attr: &str) -> Result<&str, Problem>;

    fn map_parse_attribute<T: FromStr, F>(&self, attr: &str, f: F) -> Option<Result<T, Problem>>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
        F: FnOnce(Option<&str>) -> Option<&str>;

    fn find_child_by_tag_name(&self, tag: &str) -> Result<Node, Problem>;

    fn get_name(
        &self,
        node_index_in_xml_parent: usize,
        problems: &mut Vec<HandledProblem>,
    ) -> String;

    // TODO does not return Problem if attribute missing, just None
    // fn parse_attribute<T: FromStr>(&self, attr: &str) -> Result<Option<T>, ProblemType>
    // where
    //     <T as FromStr>::Err: std::error::Error + 'static;
}

impl GetFromNode for Node<'_, '_> {
    /// Get the value of an XML attribute and parse it to the output type `T`.
    ///
    /// If the attribute is missing or it can't be parsed to `T`, a `Problem` is
    /// returned.
    fn parse_required_attribute<T: FromStr>(&self, attr: &str) -> Result<T, Problem>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
    {
        // TODO return Problem instead of ProblemType - we know where the error is anyway
        let content = self.required_attribute(attr)?;
        parse_attribute_content(self, content, attr)
    }

    /// Get the value of an XML attribute and apply the closure. If it returns
    /// None, function returns None. Otherwise, returns the result of parsing
    /// the attribute as type T.
    fn map_parse_attribute<T: FromStr, F>(&self, attr: &str, f: F) -> Option<Result<T, Problem>>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
        F: FnOnce(Option<&str>) -> Option<&str>,
    {
        // TODO Big usability problem: If a result is returned, unpacking the inner "Result" to a value is really hard...
        let content = f(self.attribute(attr))?;
        Some(parse_attribute_content(self, content, attr))
    }

    /// Get the value of an XML attribute and apply the closure. If it returns
    /// None, function returns None. Otherwise, returns the result of parsing
    /// the attribute as type T.
    fn parse_attribute<T: FromStr>(&self, attr: &str) -> Option<Result<T, Problem>>
    where
        <T as FromStr>::Err: std::error::Error + 'static,
    {
        let content = self.attribute(attr)?;
        Some(parse_attribute_content(self, content, attr))
    }

    // Returns value of an atrribute, or a ProblemType if missing.
    fn required_attribute(&self, attr: &str) -> Result<&str, Problem> {
        let content = self.attribute(attr).ok_or_else(|| {
            ProblemType::XmlAttributeMissing {
                attr: attr.to_owned(),
                tag: self.tag_name().name().to_owned(),
            }
            .at(self)
        })?;
        Ok(content)
    }

    fn find_child_by_tag_name(&self, tag: &str) -> Result<Node, Problem> {
        match self.children().find(|n| n.has_tag_name(tag)) {
            Some(n) => Ok(n),
            None => Err(ProblemType::XmlNodeMissing {
                missing: tag.to_owned(),
                parent: self.tag_name().name().to_owned(),
            }
            .at(self)),
        }
    }

    /// Get attribute 'Name', or if missing provide default and push a problem.
    fn get_name(
        &self,
        node_index_in_xml_parent: usize,
        problems: &mut Vec<HandledProblem>,
    ) -> String {
        self.parse_required_attribute("Name").unwrap_or_else(|p| {
            let default_name = format!(
                "{} {}",
                self.tag_name().name(),
                node_index_in_xml_parent + 1
            );
            p.handled_by(format!("using default name '{default_name}'"), problems);
            default_name
        })
    }
}

fn parse_attribute_content<T: FromStr>(node: &Node, content: &str, attr: &str) -> Result<T, Problem>
where
    <T as FromStr>::Err: std::error::Error + 'static,
{
    content.parse::<T>().map_err(|err| {
        ProblemType::InvalidAttribute {
            attr: attr.to_owned(),
            tag: node.tag_name().name().to_owned(),
            content: content.to_owned(),
            source: Box::new(err),
            expected_type: type_name::<T>().to_owned(),
        }
        .at(node)
    })
}

pub(crate) trait AssignOrHandle<T: Display> {
    fn assign_or_handle(self, to: &mut T, problems: &mut Vec<HandledProblem>);
}

impl<T: Display> AssignOrHandle<T> for Result<T, Problem> {
    fn assign_or_handle(self, to: &mut T, problems: &mut Vec<HandledProblem>) {
        match self {
            Ok(v) => *to = v,
            Err(p) => p.handled_by(format!("using default {}", to), problems),
        }
    }
}

pub(crate) trait XmlPosition {
    fn position(&self) -> TextPos;
}

impl XmlPosition for Node<'_, '_> {
    fn position(&self) -> TextPos {
        self.document().text_pos_at(self.range().start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn get_attribute_works() {
        let xml = r#"<tag attr="300" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        assert_eq!(
            n.parse_required_attribute("attr", &mut problems, &doc),
            Some(300u32)
        );
        assert_eq!(
            n.parse_required_attribute::<u8>("attr", &mut problems, &doc),
            None
        );
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn get_attribute_errors() {
        let xml = r#"<Geometry pos="not_a_number" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        assert_eq!(
            n.parse_required_attribute("pos", &mut problems, &doc),
            Some("not_a_number".to_string())
        );
        assert_eq!(
            n.parse_required_attribute::<i16>("pos", &mut problems, &doc),
            None
        );
        assert_eq!(problems.len(), 1);
    }
}
