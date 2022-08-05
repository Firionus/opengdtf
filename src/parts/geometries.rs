use std::collections::HashMap;

use petgraph::Direction::Incoming;
use petgraph::{graph::NodeIndex, Directed, Graph};
use roxmltree::{Document, Node};
use uuid::Uuid;

use crate::{get_string_attribute, Problem};
use crate::{node_position, ProblemAdd};

#[derive(Debug, Default)]
pub struct Geometries {
    /// Graph representing the Geometry tree.
    ///
    /// Edges point from parent to child.
    pub graph: Graph<GeometryType, (), Directed>,
    pub names: HashMap<String, NodeIndex>,
}

impl Geometries {
    /// Adds a Geometry and returns the NodeIndex of the new geometry
    ///
    /// If you want to add a top-level geometry, set parent_index to `None`.
    ///
    /// If a geometry of the same name is already present, does not do anything and returns `None`.
    pub fn add(
        &mut self,
        geometry: GeometryType,
        parent_index: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        let new_name = geometry.name().to_owned();

        if self.names.contains_key(&new_name) {
            return None;
        }

        let new_ind = self.graph.add_node(geometry);
        if let Some(parent_index) = parent_index {
            self.graph.add_edge(parent_index, new_ind, ());
        };
        self.names.insert(new_name, new_ind);
        Some(new_ind)
    }
}

#[derive(Debug)]
pub struct Offsets {
    normal: HashMap<u32, u32>, // dmx_break => offset
    overwrite: Offset,
}

impl Offsets {
    pub fn new(overwrite: Offset) -> Self {
        Offsets {
            normal: HashMap::new(),
            overwrite,
        }
    }
}

#[derive(Debug)]
pub struct Offset {
    dmx_break: u32,
    offset: u32,
}

#[derive(Debug)]
pub enum GeometryType {
    Geometry {
        name: String,
    },
    Reference {
        name: String,
        reference: NodeIndex,
        offsets: Offsets,
    },
}

impl GeometryType {
    pub fn name(&self) -> &str {
        match self {
            GeometryType::Geometry { name } | GeometryType::Reference { name, .. } => name,
        }
    }
}

pub fn parse_geometries(
    geometries: &mut Geometries,
    ft: &Node,
    problems: &mut Vec<Problem>,
    doc: &Document,
) {
    let g = match ft.children().find(|n| n.has_tag_name("Geometries")) {
        Some(g) => g,
        None => {
            problems.push(Problem::XmlNodeMissing {
                missing: "Geometries".to_owned(),
                parent: "FixtureType".to_owned(),
                pos: node_position(ft, doc),
            });
            return;
        }
    };

    let top_level_geometries: Vec<Node> = g.children().filter(|n| n.is_element()).collect();
    let mut top_level_geometry_graph_indices: Vec<NodeIndex> = vec![];

    // First, add top-level geometries. These must exist so a GeometryReference
    // later on can be linked to a NodeIndex.
    top_level_geometries.iter().for_each(|n| {
        match n.tag_name().name() {
            "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo" | "FilterShaper"
            | "Beam" | "MediaServerLayer" | "MediaServerCamera" | "MediaServerMaster"
            | "Display" | "Laser" | "WiringObject" | "Inventory" | "Structure" | "Support"
            | "Magnet" => {
                let name = geometry_name(n, problems, doc, &geometries.names);
                let geometry = GeometryType::Geometry { name };
                let i = geometries
                    .add(geometry, None)
                    .expect("Geometry Names must be unique at this point");
                top_level_geometry_graph_indices.push(i);
            }
            "GeometryReference" => problems.push(Problem::UnexpectedTopLevelGeometryReference(
                node_position(n, doc),
            )),
            tag => problems.push(Problem::UnexpectedXmlNode(
                tag.to_owned(),
                node_position(n, doc),
            )),
        };
    });

    // Next, add non-top-level geometries.
    top_level_geometries.iter().enumerate().for_each(|(i, n)| {
        let graph_index = top_level_geometry_graph_indices[i];
        add_children(n, graph_index, geometries, problems, doc);
    });
}

/// Gets a Geometry Name and provides unique default value if invalid
fn geometry_name(
    n: &Node,
    problems: &mut Vec<Problem>,
    doc: &Document,
    geometry_names: &HashMap<String, NodeIndex>,
) -> String {
    let mut name = get_string_attribute(n, "Name", problems, doc)
        .unwrap_or_else(|| format!("No Name {}", Uuid::new_v4()));

    if geometry_names.contains_key(&name) {
        problems.push(Problem::DuplicateGeometryName(
            name.to_owned(),
            node_position(n, doc),
        ));
        name = format!("{} {}", name, Uuid::new_v4())
    }

    name
}

fn add_children(
    parent_xml: &Node,
    parent_tree: NodeIndex,
    geometries: &mut Geometries,
    problems: &mut Vec<Problem>,
    doc: &Document,
) {
    parent_xml
        .children()
        .filter(|n| n.is_element())
        .for_each(|n| {
            match n.tag_name().name() {
                "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo"
                | "FilterShaper" | "Beam" | "MediaServerLayer" | "MediaServerCamera"
                | "MediaServerMaster" | "Display" | "Laser" | "WiringObject" | "Inventory"
                | "Structure" | "Support" | "Magnet" => {
                    let name = geometry_name(&n, problems, doc, &geometries.names);
                    let geometry = GeometryType::Geometry { name };
                    let i = geometries
                        .add(geometry, Some(parent_tree))
                        .expect("Geometry Names must be unique at this point");
                    add_children(&n, i, geometries, problems, doc);
                }
                "GeometryReference" => {
                    let name = geometry_name(&n, problems, doc, &geometries.names);
                    let ref_ind = get_string_attribute(&n, "Geometry", problems, doc)
                        .and_then(|refname| {
                            if refname.contains('.') {
                                problems.push_then_none(Problem::NonTopLevelGeometryReferenced(
                                    refname,
                                    node_position(&n, doc),
                                ))
                            } else {
                                Some(refname)
                            }
                        })
                        .and_then(|refname| {
                            find_geometry(&refname, geometries).or_else(|| {
                                problems.push_then_none(Problem::UnknownGeometry(
                                    refname,
                                    node_position(&n, doc),
                                ))
                            })
                        });
                    let offsets = parse_reference_offsets(&n, problems, doc);

                    if let (Some(ref_ind), Some(offsets)) = (ref_ind, offsets) {
                        let geometry = GeometryType::Reference {
                            name,
                            offsets,
                            reference: ref_ind,
                        };
                        geometries
                            .add(geometry, Some(parent_tree))
                            .expect("Geometry Names must be unique at this point");
                    };
                }
                tag => problems.push(Problem::UnexpectedXmlNode(
                    tag.to_owned(),
                    node_position(&n, doc),
                )),
            };
        });
}

fn parse_reference_offsets(
    &n: &Node,
    problems: &mut Vec<Problem>,
    doc: &Document,
) -> Option<Offsets> {
    let mut nodes = n
        .children()
        .filter(|n| n.tag_name().name() == "Break")
        .rev(); // start at last element, which we assume provides the Overwrite offset

    let last_break = nodes.next().or_else(|| {
        problems.push_then_none(Problem::XmlNodeMissing {
            missing: "Break".to_owned(),
            parent: "GeometryReference".to_owned(),
            pos: node_position(&n, doc),
        })
    })?;

    let mut dmx_break = get_u32_attribute(&last_break, "DMXBreak", problems, doc)?;
    let mut offset = get_u32_attribute(&last_break, "DMXOffset", problems, doc)?;

    let overwrite = Offset { dmx_break, offset };

    let mut offsets = Offsets::new(overwrite);

    loop {
        offsets.normal.insert(dmx_break, offset); // lower breaks are overwritten by higher ones, being inserted later
        // TODO if a break occurs twice, except in the last one defining overwrite, there should be a problem

        let current_element = match nodes.next() {
            Some(e) => e,
            None => break,
        };

        dmx_break = get_u32_attribute(&current_element, "DMXBreak", problems, doc)?;
        offset = get_u32_attribute(&current_element, "DMXOffset", problems, doc)?;
    }

    Some(offsets)
}

fn get_u32_attribute(
    n: &Node,
    attr: &str,
    problems: &mut Vec<Problem>,
    doc: &Document,
) -> Option<u32> {
    match get_string_attribute(n, attr, problems, doc)?.parse() {
        Ok(v) => Some(v),
        Err(err) => problems.push_then_none(Problem::InvalidInteger {
            attr: attr.to_owned(),
            tag: n.tag_name().name().to_owned(),
            pos: node_position(n, doc),
            err,
        }),
    }
}

fn find_geometry(name: &str, geometries: &Geometries) -> Option<NodeIndex> {
    let mut rev_path = name.split('.').rev();

    rev_path
        .next()
        .and_then(|element_name| geometries.names.get(element_name))
        .map(|i| i.to_owned())
        .and_then(|i| {
            // validate path by going backwards up the graph
            let mut current_ind = i;
            loop {
                let mut parents = geometries.graph.neighbors_directed(current_ind, Incoming);
                match parents.next() {
                    None => match rev_path.next() {
                        None => break Some(i),
                        Some(_parent_name) => break None,
                    },
                    Some(parent_ind) => {
                        let parent_name = geometries.graph[parent_ind].name();
                        if Some(parent_name) != rev_path.next() {
                            return None;
                        }
                        current_ind = parent_ind;
                    }
                };
                // assert!(parents.next() == None) // Graph is tree, so each node only has one parent
            }
        })
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::*;

    #[test]
    fn get_u32_attribute_works() {
        let xml = r#"<tag attr="3" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let n = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        assert_eq!(get_u32_attribute(&n, "attr", &mut problems, &doc), Some(3));
    }

    #[test]
    fn reference_offsets() {
        let offsets = Offsets::new(Offset {
            dmx_break: 1,
            offset: 1,
        });
        assert_eq!(offsets.normal.len(), 0);
        assert_eq!(offsets.overwrite.dmx_break, 1);
        assert_eq!(offsets.overwrite.offset, 1);
    }

    #[test]
    fn geometries_default_is_empty() {
        let geometries = Geometries::default();
        assert_eq!(geometries.graph.node_count(), 0);
        assert_eq!(geometries.names.len(), 0);
    }

    #[test]
    fn find_geometry_works() {
        let mut geometries = Geometries::default();

        let a = geometries
            .add(
                GeometryType::Geometry {
                    name: "a".to_owned(),
                },
                None,
            )
            .unwrap();
        let b = geometries
            .add(
                GeometryType::Geometry {
                    name: "b".to_owned(),
                },
                None,
            )
            .unwrap();
        let b1 = geometries
            .add(
                GeometryType::Geometry {
                    name: "b1".to_owned(),
                },
                Some(b),
            )
            .unwrap();
        let b1a = geometries
            .add(
                GeometryType::Geometry {
                    name: "b1a".to_owned(),
                },
                Some(b1),
            )
            .unwrap();

        assert_eq!(find_geometry("a", &geometries), Some(a));
        assert_eq!(find_geometry("b", &geometries), Some(b));
        assert_eq!(find_geometry("b.b1", &geometries), Some(b1));
        assert_eq!(find_geometry("b.b1.b1a", &geometries), Some(b1a));

        // can't reference directly without parent, even though it's clear which element it would be
        assert_eq!(find_geometry("b1", &geometries), None);
        assert_eq!(find_geometry("b1a", &geometries), None);

        // nonexistent elements
        assert_eq!(find_geometry("c", &geometries), None);
        assert_eq!(find_geometry("a.c", &geometries), None);

        // nonexistent paths, though end element exists
        assert_eq!(find_geometry("a.a", &geometries), None);
        assert_eq!(find_geometry("c.a", &geometries), None);

        // parent missing in path
        assert_eq!(find_geometry("b1.b1a", &geometries), None);
    }

    #[test]
    fn geometries_smoke_test() {
        let ft_str = r#"
<FixtureType>
    <Geometries>
        <Geometry Name="AbstractElement" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
        <Geometry Name="Main" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
            <GeometryReference Geometry="AbstractElement" Name="Element 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Break DMXBreak="1" DMXOffset="1"/>
                <Break DMXBreak="2" DMXOffset="1"/>
                <Break DMXBreak="1" DMXOffset="1"/>
            </GeometryReference>
            <GeometryReference Geometry="AbstractElement" Name="Element 2" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Break DMXBreak="1" DMXOffset="3"/>
                <Break DMXBreak="2" DMXOffset="4"/>
                <Break DMXBreak="1" DMXOffset="2"/>
            </GeometryReference>
        </Geometry>
    </Geometries>
</FixtureType>
        "#;

        let (problems, geometries) = run_parse_geometries(ft_str);

        assert!(problems.is_empty());
        assert_eq!(geometries.graph.node_count(), 4);

        let element_2 = &geometries.graph[geometries.names["Element 2"]];
        if let GeometryType::Reference { name, reference, offsets } = element_2 {
            assert_eq!(name, "Element 2");
            assert_eq!(offsets.overwrite.dmx_break, 1);
            assert_eq!(offsets.overwrite.offset, 2);
            assert_eq!(offsets.normal[&1], 3);
            assert_eq!(offsets.normal[&2], 4);
            assert_eq!(geometries.graph[reference.to_owned()].name(), "AbstractElement");
        } else {
            panic!("shouldn't happen")
        };
    }

    #[test]
    fn geometries_top_level_name_missing() {
        let ft_str = r#"
<FixtureType>
    <Geometries>
        <Geometry Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
        <Geometry Name="Main" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
        </Geometry>
    </Geometries>
</FixtureType>
        "#;

        let (problems, geometries) = run_parse_geometries(ft_str);

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::XmlAttributeMissing { .. }));

        assert_eq!(geometries.graph.node_count(), 2);

        geometries
            .graph
            .raw_nodes()
            .iter()
            .for_each(|n| assert!(n.weight.name() != ""));

        let name_of_broken_node = geometries.graph.raw_nodes()[0].weight.name();
        assert_is_uuid_and_not_nil(name_of_broken_node);
    }

    #[test]
    fn geometries_duplicate_names() {
        let ft_str = r#"
<FixtureType>
    <Geometries>
        <Geometry Name="AbstractElement" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
        <Geometry Name="Main" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
            <GeometryReference Geometry="AbstractElement" Name="Element 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Break DMXBreak="1" DMXOffset="1"/>
                <Break DMXBreak="2" DMXOffset="1"/>
                <Break DMXBreak="1" DMXOffset="1"/>
            </GeometryReference>
            <GeometryReference Geometry="AbstractElement" Name="Element 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Break DMXBreak="1" DMXOffset="3"/>
                <Break DMXBreak="2" DMXOffset="3"/>
                <Break DMXBreak="1" DMXOffset="2"/>
            </GeometryReference>
        </Geometry>
    </Geometries>
</FixtureType>
        "#;

        let (problems, geometries) = run_parse_geometries(ft_str);

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::DuplicateGeometryName(..)));

        let name_of_broken_node = geometries.graph.raw_nodes()[3].weight.name();
        assert_is_uuid_and_not_nil(name_of_broken_node);
    }

    fn run_parse_geometries(ft_str: &str) -> (Vec<Problem>, Geometries) {
        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        let mut geometries = Geometries::default();
        parse_geometries(&mut geometries, &ft, &mut problems, &doc);
        (problems, geometries)
    }

    fn assert_is_uuid_and_not_nil(s: &str) {
        let uuid_pattern =
            Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
        assert!(uuid_pattern.is_match(s));
        let uuid_nil_pattern = Regex::new(r"00000000-0000-0000-0000-000000000000").unwrap();
        assert!(!uuid_nil_pattern.is_match(s));
    }
}
