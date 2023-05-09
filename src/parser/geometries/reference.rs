use crate::{
    geometry::{Geometry, Offset, Offsets, Type},
    parser::{parse_xml::GetXmlAttribute, problems::HandleProblem},
    types::{dmx_break::Break, name::Name},
    Problem, ProblemAt, Problems,
};

use super::GeometriesParser;
use petgraph::graph::NodeIndex;
use roxmltree::Node;

use std::collections::hash_map::Entry::Vacant;

#[derive(Debug)]
pub(super) struct DeferredReference<'a> {
    referencing_node: Node<'a, 'a>,
    name: Name,
    referenced: Name,
}

impl<'a> GeometriesParser<'a> {
    pub(super) fn named_geometry_reference(
        &mut self,
        n: Node<'a, 'a>,
        name: Name,
    ) -> Option<Geometry> {
        let offsets = parse_reference_offsets(n, &name, self.problems);

        let geometry = Geometry {
            name: name.clone(),
            t: Type::Reference { offsets },
        };

        let ref_string = n
            .parse_required_attribute::<Name>("Geometry")
            .ok_or_handled_by("not parsing node", self.problems)?;
        self.references.push_back(DeferredReference {
            referencing_node: n,
            name,
            referenced: ref_string,
        });

        Some(geometry)
    }

    pub(super) fn parse_references(&mut self) {
        while let Some(d) = self.references.pop_front() {
            let referenced =
                match self.get_index_of_referenced_geometry(d.referencing_node, d.referenced) {
                    Ok(v) => v,
                    Err(p) => {
                        p.handled_by("not adding reference", self.problems);
                        continue;
                    }
                };
            let reference = match self.geometries.get_index(&d.name) {
                Some(v) => v,
                None => {
                    Problem::Unexpected("geometry reference node never added".into())
                        .at(&d.referencing_node)
                        .handled_by("not adding reference", self.problems);
                    continue;
                }
            };
            if let Err(err) = self
                .geometries
                .add_template_relationship(referenced, reference)
            {
                Problem::InvalidGeometryReference(err)
                    .at(&d.referencing_node)
                    .handled_by("not adding reference", self.problems);
                continue;
            };
        }
    }

    fn get_index_of_referenced_geometry(
        &mut self,
        n: Node,
        ref_string: Name,
    ) -> Result<NodeIndex, ProblemAt> {
        let ref_ind = self
            .geometries
            .get_index(&ref_string)
            .ok_or_else(|| Problem::UnknownGeometry(ref_string.clone()).at(&n))?;
        Ok(ref_ind)
    }
}

fn parse_reference_offsets(n: Node, name: &Name, problems: &mut Problems) -> Offsets {
    let mut offsets = Offsets::default();

    let mut nodes = n
        .children()
        .filter(|n| n.tag_name().name() == "Break")
        .rev(); // start at last element, which provides the Overwrite offset if present

    if let Some(last_break) = nodes.next() {
        offsets.overwrite = parse_break(last_break)
            .ok_or_handled_by("ignoring node and setting overwrite to None", problems);
    };

    for n in nodes {
        let Some(Offset { dmx_break, offset }) = parse_break(n).ok_or_handled_by("ignoring node", problems) else {
            continue
        };

        if offsets.normal.contains_key(&dmx_break) {
            Problem::DuplicateDmxBreak {
                duplicate_break: dmx_break,
                geometry_reference: name.to_owned(),
            }
            .at(&n)
            .handled_by("overwriting previous value", problems)
        }
        offsets.normal.insert(dmx_break, offset);
    }

    // add overwrite to normal if break not already present
    if let Some(Offset { dmx_break, offset }) = offsets.overwrite {
        if let Vacant(entry) = offsets.normal.entry(dmx_break) {
            entry.insert(offset);
        }
    }

    offsets
}

fn parse_break(n: Node) -> Result<Offset, ProblemAt> {
    Ok(Offset {
        dmx_break: n
            .parse_attribute("DMXBreak")
            .unwrap_or_else(|| Ok(Break::default()))?,
        offset: n.parse_attribute("DMXOffset").unwrap_or(Ok(1))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_break_node(xml: &str) -> Result<Offset, ProblemAt> {
        let doc = roxmltree::Document::parse(xml).unwrap();
        parse_break(doc.root_element())
    }

    #[test]
    fn test_parse_break_node() {
        assert!(matches!(
            parse_break_node(r#"<Break DMXBreak="1" DMXOffset="1" />"#),
            Ok(Offset{dmx_break, offset}) if dmx_break.value() == &1u16 && offset == 1
        ));

        assert!(matches!(
            parse_break_node(r#"<Break DMXBreak="-1" DMXOffset="1" />"#),
            Err(p @ ProblemAt { .. }) if matches!(p.problem(), Problem::InvalidAttribute{attr, ..} if attr=="DMXBreak")
        ));
    }

    fn run_parse_reference_offsets(xml: &str) -> (Problems, Offsets) {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Problems = vec![];
        let offsets = parse_reference_offsets(
            n,
            &"arbitrary name for testing".try_into().unwrap(),
            &mut problems,
        );
        (problems, offsets)
    }

    #[test]
    fn non_overlapping_break_overwrite() {
        let xml = r#"
<GeometryReference>
    <Break DMXBreak="1" DMXOffset="6"/>
    <Break DMXBreak="2" DMXOffset="5"/>
    <Break DMXBreak="3" DMXOffset="4"/>
</GeometryReference>"#;
        let (problems, offsets) = run_parse_reference_offsets(xml);
        assert_eq!(problems.len(), 0);
        assert_eq!(
            offsets.overwrite,
            Some(Offset {
                dmx_break: 3.try_into().unwrap(),
                offset: 4
            })
        );
        assert_eq!(offsets.normal.len(), 3);
        assert_eq!(offsets.normal.get(&1.try_into().unwrap()).unwrap(), &6);
        assert_eq!(offsets.normal.get(&2.try_into().unwrap()).unwrap(), &5);
        assert_eq!(offsets.normal.get(&3.try_into().unwrap()).unwrap(), &4);
    }

    #[test]
    fn must_not_skip_nodes_around_bad_one() {
        let xml = r#"
<GeometryReference>
    <Break DMXBreak="1" DMXOffset="6"/>
    <Break DMXBreak="-1" DMXOffset="5"/>
    <Break DMXBreak="3" DMXOffset="4"/>
</GeometryReference>"#;
        let (problems, offsets) = run_parse_reference_offsets(xml);
        assert_eq!(problems.len(), 1);
        assert_eq!(
            offsets.overwrite,
            Some(Offset {
                dmx_break: 3.try_into().unwrap(),
                offset: 4
            })
        );
        assert_eq!(offsets.normal.len(), 2);
        assert_eq!(offsets.normal.get(&1.try_into().unwrap()).unwrap(), &6);
        assert_eq!(offsets.normal.get(&3.try_into().unwrap()).unwrap(), &4);
    }

    #[test]
    fn handles_overwrite_with_missing_argument() {
        let xml = r#"
<GeometryReference>
    <Break DMXBreak="1" DMXOffset="6"/>
    <Break DMXBreak="2" DMXOffset="5"/>
    <Break MissingDMXBreak="3" DMXOffset="4"/>
</GeometryReference>"#;
        let (problems, offsets) = run_parse_reference_offsets(xml);
        assert_eq!(problems.len(), 0);
        assert_eq!(
            offsets.overwrite,
            Some(Offset {
                dmx_break: 1.try_into().unwrap(),
                offset: 4
            })
        );
        assert_eq!(offsets.normal.len(), 2);
        assert_eq!(offsets.normal.get(&1.try_into().unwrap()).unwrap(), &6);
        assert_eq!(offsets.normal.get(&2.try_into().unwrap()).unwrap(), &5);
    }

    #[test]
    fn handles_overwrite_with_invalid_argument() {
        let xml = r#"
<GeometryReference>
    <Break DMXBreak="1" DMXOffset="6"/>
    <Break DMXBreak="2" DMXOffset="5"/>
    <Break DMXBreak="0" DMXOffset="4"/> <!-- breaks must be bigger than 0 -->
</GeometryReference>"#;
        let (problems, offsets) = run_parse_reference_offsets(xml);
        assert_eq!(problems.len(), 1);
        assert_eq!(offsets.overwrite, None);
        assert_eq!(offsets.normal.len(), 2);
        assert_eq!(offsets.normal.get(&1.try_into().unwrap()).unwrap(), &6);
        assert_eq!(offsets.normal.get(&2.try_into().unwrap()).unwrap(), &5);
    }

    #[test]
    fn duplicate_break() {
        let xml = r#"
<GeometryReference>
    <Break DMXBreak="1" DMXOffset="1"/>
    <Break DMXBreak="2" DMXOffset="2"/>
    <Break DMXBreak="2" DMXOffset="3"/>  <!-- This is a duplicate break -->
    <Break DMXBreak="1" DMXOffset="4"/>  <!-- This is not a duplicate break, since it occurs in the last element, which is overwrite -->
</GeometryReference>"#;
        let (mut problems, offsets) = run_parse_reference_offsets(xml);
        assert_eq!(problems.len(), 1);
        assert!(matches!(
            problems.pop().unwrap().problem(),
            Problem::DuplicateDmxBreak {
                duplicate_break,
                ..
            }
        if duplicate_break == &Break::try_from(2).unwrap()));
        assert_eq!(offsets.normal.get(&2.try_into().unwrap()).unwrap(), &2); // higher element takes precedence
    }

    #[test]
    fn empty_reference_offsets() {
        let xml = r#"<GeometryReference />"#;
        let (problems, offsets) = run_parse_reference_offsets(xml);
        assert_eq!(problems.len(), 0);
        assert_eq!(offsets, Offsets::default());
    }
}
