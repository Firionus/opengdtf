use roxmltree::Node;

use crate::{Problem, ProblemAt};

pub(crate) trait GetXmlNode {
    fn find_required_child(&self, tag: &str) -> Result<Node, ProblemAt>;
}

impl GetXmlNode for Node<'_, '_> {
    /// Find the first child node with the given tag name.
    fn find_required_child(&self, tag: &str) -> Result<Node, ProblemAt> {
        match self.children().find(|n| n.has_tag_name(tag)) {
            Some(n) => Ok(n),
            None => Err(Problem::XmlNodeMissing {
                missing: tag.to_owned(),
                parent: self.tag_name().name().to_owned(),
            }
            .at(self)),
        }
    }
}
