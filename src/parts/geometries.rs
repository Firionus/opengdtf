use std::collections::hash_map::Entry::Vacant;
use std::collections::HashMap;

use petgraph::Direction::Incoming;
use petgraph::{graph::NodeIndex, Directed, Graph};
use roxmltree::{Document, Node};
use uuid::Uuid;

use crate::utils::GetAttribute;
use crate::Problem;
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
    /// If you want to add a top-level geometry, set parent_index to `None`. If
    /// a geometry of the same name is already present, does not do anything and
    /// returns `None`.
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

    /// Find the NodeIndex of a Geometry by its unique `Name`.
    pub fn find(&self, name: &str) -> Option<NodeIndex> {
        self.names.get(name).map(|i| i.to_owned())
    }

    /// Checks if the Geometry with given `NodeIndex` `i` is a top-level geometry.
    ///
    /// If geometry with index `i` doesn't exist, `true` is returned.
    pub fn is_top_level(&self, i: NodeIndex) -> bool {
        match self.graph.edges_directed(i, Incoming).next() {
            None => true,
            Some(_) => false,
        }
    }
}

// TODO When Channel parsing is implemented, there needs to be a validation that
// each `Offsets` in a `GeometryReference` contains the breaks required by
// channels operating on the referenced geometry. No more breaks are allowed to
// be serialized (see GDTF 1.2 page 39), but I think having them in the struct
// isn't bad.
#[derive(Debug, PartialEq)]
pub struct Offsets {
    normal: HashMap<u16, u16>, // dmx_break => offset // TODO same validations as Offset
    overwrite: Option<Offset>,
}

impl Offsets {
    pub fn new() -> Self {
        Offsets {
            normal: HashMap::new(),
            overwrite: None,
        }
    }
}

impl Default for Offsets {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq)]
pub struct Offset {
    dmx_break: u16, // TODO 0 disallowed, is there an upper limit on breaks?
    offset: u16,    // TODO more than 512 disallowed, 0 disallowed? negative disallowed?
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
    let mut name = n
        .parse_required_attribute("Name", problems, doc)
        .unwrap_or_else(|| format!("No Name {}", Uuid::new_v4()));

    if geometry_names.contains_key(&name) {
        problems.push(Problem::DuplicateGeometryName(
            // TODO GDTF Share contains files with duplicate Geometry names, e.g. Robe Tetra 2
            // It seems like "Unique Geometry Name" was only applied relative to a top-level Geometry of a certain DMX Mode?
            // Nowadays, the builder seems to disallow duplicate Geometry Names
            // if you try entering them, but doesn't complain about legacy files.
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
                    let ref_ind = n
                        .parse_required_attribute::<String>("Geometry", problems, doc)
                        .and_then(|refname| {
                            geometries.find(&refname).or_else(|| {
                                problems.push_then_none(Problem::UnknownGeometry(
                                    refname,
                                    node_position(&n, doc),
                                ))
                            })
                        })
                        .and_then(|i| {
                            if !geometries.is_top_level(i) {
                                problems.push_then_none(Problem::NonTopLevelGeometryReferenced(
                                    geometries.graph[i].name().to_owned(),
                                    node_position(&n, doc),
                                ))
                            } else {
                                Some(i)
                            }
                        });
                    let offsets = parse_reference_offsets(&n, problems, doc);

                    if let Some(ref_ind) = ref_ind {
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

fn parse_reference_offsets(&n: &Node, problems: &mut Vec<Problem>, doc: &Document) -> Offsets {
    let mut nodes = n
        .children()
        .filter(|n| n.tag_name().name() == "Break")
        .rev(); // start at last element, which provides the Overwrite offset if present

    let mut offsets = Offsets::new();

    nodes.next().and_then(|last_break| {
        let dmx_break = last_break.parse_required_attribute("DMXBreak", problems, doc)?;
        let offset = last_break.parse_required_attribute("DMXOffset", problems, doc)?;
        offsets.overwrite = Some(Offset { dmx_break, offset });
        Some(())
    });

    for element in nodes {
        if let (Some(dmx_break), Some(offset)) = (
            element.parse_required_attribute("DMXBreak", problems, doc),
            element.parse_required_attribute("DMXOffset", problems, doc),
        ) {
            if offsets.normal.contains_key(&dmx_break) {
                problems.push(Problem::DuplicateDmxBreak(
                    dmx_break,
                    node_position(&element, doc),
                ));
            }

            offsets.normal.insert(dmx_break, offset); // overwrite whether occupied or vacant
        };
    }

    // add overwrite to normal if break not present
    if let Some(Offset { dmx_break, offset }) = offsets.overwrite {
        if let Vacant(entry) = offsets.normal.entry(dmx_break) {
            entry.insert(offset);
        }
    }

    offsets
}

#[cfg(test)]
mod tests {
    use std::ops::Not;

    use regex::Regex;

    use super::*;

    #[cfg(test)]
    mod parse_reference_offsets {
        use super::*;

        #[test]
        fn basic_test() {
            let xml = r#"
    <GeometryReference>
        <Break DMXBreak="1" DMXOffset="1"/>
        <Break DMXBreak="2" DMXOffset="2"/>
        <Break DMXBreak="1" DMXOffset="4"/>
    </GeometryReference>"#;
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 0);
            assert_eq!(
                offsets.overwrite,
                Some(Offset {
                    dmx_break: 1,
                    offset: 4
                })
            );
            assert_eq!(offsets.normal.len(), 2);
            assert_eq!(offsets.normal[&1], 1);
            assert_eq!(offsets.normal[&2], 2);
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
                    dmx_break: 3,
                    offset: 4
                })
            );
            assert_eq!(offsets.normal.len(), 3);
            assert_eq!(offsets.normal[&1], 6);
            assert_eq!(offsets.normal[&2], 5);
            assert_eq!(offsets.normal[&3], 4);
        }

        #[test]
        fn must_not_skip_nodes_around_bad_one() {
            let xml = r#"
    <GeometryReference>
        <Break DMXBreak="1" DMXOffset="6"/>
        <Break MissingDMXBreak="2" DMXOffset="5"/>
        <Break DMXBreak="3" DMXOffset="4"/>
    </GeometryReference>"#;
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 1);
            assert_eq!(
                offsets.overwrite,
                Some(Offset {
                    dmx_break: 3,
                    offset: 4
                })
            );
            assert_eq!(offsets.normal.len(), 2);
            assert_eq!(offsets.normal[&1], 6);
            assert_eq!(offsets.normal[&3], 4);
        }

        #[test]
        fn handles_broken_overwrite() {
            let xml = r#"
    <GeometryReference>
        <Break DMXBreak="1" DMXOffset="6"/>
        <Break DMXBreak="2" DMXOffset="5"/>
        <Break MissingDMXBreak="3" DMXOffset="4"/>
    </GeometryReference>"#;
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 1);
            assert_eq!(offsets.overwrite, None);
            assert_eq!(offsets.normal.len(), 2);
            assert_eq!(offsets.normal[&1], 6);
            assert_eq!(offsets.normal[&2], 5);
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
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 1);
            assert!(matches!(problems[0], Problem::DuplicateDmxBreak(..)));
            assert_eq!(offsets.normal[&2], 2); // higher element takes precedence
        }

        #[test]
        fn empty_reference_offsets() {
            let xml = r#"<GeometryReference />"#;
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 0);
            assert_eq!(offsets, Offsets::new());
        }

        fn run_parse_reference_offsets(xml: &str) -> (Vec<Problem>, Offsets) {
            let doc = roxmltree::Document::parse(xml).unwrap();
            let n = doc.root_element();
            let mut problems: Vec<Problem> = vec![];
            let offsets = parse_reference_offsets(&n, &mut problems, &doc);
            (problems, offsets)
        }
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

        assert_eq!(geometries.find("a"), Some(a));
        assert_eq!(geometries.find("b"), Some(b));
        assert_eq!(geometries.find("b1"), Some(b1));
        assert_eq!(geometries.find("b1a"), Some(b1a));

        // nonexistent elements
        assert_eq!(geometries.find("c"), None);
        assert_eq!(geometries.find("a.a"), None);
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
        if let GeometryType::Reference {
            name,
            reference,
            offsets,
        } = element_2
        {
            assert_eq!(name, "Element 2");
            assert_eq!(
                offsets.overwrite,
                Some(Offset {
                    dmx_break: 1,
                    offset: 2
                })
            );
            assert_eq!(offsets.normal[&1], 3);
            assert_eq!(offsets.normal[&2], 4);
            assert_eq!(
                geometries.graph[reference.to_owned()].name(),
                "AbstractElement"
            );
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

    #[test]
    fn geometry_reference_to_non_top_level_geometry() {
        let ft_str = r#"
<FixtureType>
    <Geometries>
        <Geometry Name="Main 2" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
            <Geometry Name="AbstractElement" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
        </Geometry>
        <Geometry Name="Main" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
            <GeometryReference Geometry="AbstractElement" Name="Element 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}" />
        </Geometry>
    </Geometries>
</FixtureType>
        "#;

        let (problems, geometries) = run_parse_geometries(ft_str);

        assert_eq!(problems.len(), 1);
        assert!(matches!(
            problems[0],
            Problem::NonTopLevelGeometryReferenced(..)
        ));

        assert_eq!(geometries.graph.node_count(), 3);
        assert!(geometries.names.contains_key("Element 1").not());
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
