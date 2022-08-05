use std::collections::HashMap;
use std::ops::Index;

use petgraph::Direction::Incoming;
use petgraph::{graph::NodeIndex, Directed, Graph};
use roxmltree::{Document, Node};
use uuid::Uuid;

use crate::{get_string_attribute, Problem};
use crate::{node_position, ProblemAdd};

/// Graph representing the Geometry tree.
///
/// Edges point from parent to child.
pub type Geometries = Graph<GeometryType, (), Directed>;

#[derive(Debug)]
pub enum GeometryType {
    Geometry {
        name: String,
    },
    Reference {
        name: String,
        reference: NodeIndex,
        break_offsets: (),
    },
}

impl GeometryType {
    fn name(&self) -> &str {
        match self {
            GeometryType::Geometry { name } | GeometryType::Reference { name, .. } => name,
        }
    }
}

pub fn parse_geometries(
    geometries: &mut Geometries,
    geometry_names: &mut HashMap<String, NodeIndex>,
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
                let name = geometry_name(n, problems, doc, geometry_names);
                let new_graph_node = geometries.add_node(GeometryType::Geometry {
                    name: name.to_owned(),
                });
                top_level_geometry_graph_indices.push(new_graph_node);
                geometry_names.insert(name, new_graph_node);
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
        add_children(n, graph_index, geometries, problems, doc, geometry_names);
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
    geometry_names: &mut HashMap<String, NodeIndex>,
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
                    let name = geometry_name(&n, problems, doc, geometry_names);
                    let ind = geometries.add_node(GeometryType::Geometry {
                        name: name.to_owned(),
                    });
                    geometries.add_edge(parent_tree, ind, ());
                    geometry_names.insert(name, ind);
                    add_children(&n, ind, geometries, problems, doc, geometry_names);
                }
                "GeometryReference" => {
                    let name = geometry_name(&n, problems, doc, geometry_names);
                    if let Some(ref_ind) = get_string_attribute(&n, "Geometry", problems, doc)
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
                            find_geometry(&refname, geometries, geometry_names).or_else(|| {
                                problems.push_then_none(Problem::UnknownGeometry(
                                    refname,
                                    node_position(&n, doc),
                                ))
                            })
                        })
                    {
                        let new_ind = geometries.add_node(GeometryType::Reference {
                            name: name.to_owned(),
                            break_offsets: (),
                            reference: ref_ind,
                        });
                        geometries.add_edge(parent_tree, new_ind, ());
                        geometry_names.insert(name, new_ind);
                        // TODO code duplication with other geometry adds, but there's a different constructor in the middle
                    };
                }
                tag => problems.push(Problem::UnexpectedXmlNode(
                    tag.to_owned(),
                    node_position(&n, doc),
                )),
            };
        });
}

fn find_geometry(
    name: &str,
    geometries: &Geometries,
    geometry_names: &HashMap<String, NodeIndex>,
) -> Option<NodeIndex> {
    let mut rev_path = name.split('.').rev();

    rev_path
        .next()
        .and_then(|element_name| geometry_names.get(element_name))
        .map(|i| i.to_owned())
        .and_then(|i| {
            // validate path by going backwards up the graph
            let mut current_ind = i;
            loop {
                let mut parents = geometries.neighbors_directed(current_ind, Incoming);
                match parents.next() {
                    None => match rev_path.next() {
                        None => break Some(i),
                        Some(_parent_name) => break None,
                    },
                    Some(parent_ind) => {
                        let parent_name = geometries.index(parent_ind).name();
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
    fn find_geometry_works() {
        let mut geometries: Geometries = Graph::new();
        let mut geometry_names: HashMap<String, NodeIndex> = HashMap::new();

        // TODO this kind of setup should be moved to an impl Gdtf (gdtf.add_geometry(geom, parent_index))
        // together with its error handling that is currently in the parsing code
        let a = geometries.add_node(GeometryType::Geometry {
            name: "a".to_owned(),
        });
        let b = geometries.add_node(GeometryType::Geometry {
            name: "b".to_owned(),
        });
        let b1 = geometries.add_node(GeometryType::Geometry {
            name: "b1".to_owned(),
        });
        let b1a = geometries.add_node(GeometryType::Geometry {
            name: "b1a".to_owned(),
        });
        geometries.add_edge(b, b1, ());
        geometries.add_edge(b1, b1a, ());
        geometry_names.insert("a".to_owned(), a);
        geometry_names.insert("b".to_owned(), b);
        geometry_names.insert("b1".to_owned(), b1);
        geometry_names.insert("b1a".to_owned(), b1a);

        assert_eq!(find_geometry("a", &geometries, &geometry_names), Some(a));
        assert_eq!(find_geometry("b", &geometries, &geometry_names), Some(b));
        assert_eq!(
            find_geometry("b.b1", &geometries, &geometry_names),
            Some(b1)
        );
        assert_eq!(
            find_geometry("b.b1.b1a", &geometries, &geometry_names),
            Some(b1a)
        );

        // can't reference directly without parent, even though it's clear which element it would be
        assert_eq!(find_geometry("b1", &geometries, &geometry_names), None);
        assert_eq!(find_geometry("b1a", &geometries, &geometry_names), None);

        // nonexistent elements
        assert_eq!(find_geometry("c", &geometries, &geometry_names), None);
        assert_eq!(find_geometry("a.c", &geometries, &geometry_names), None);

        // nonexistent paths, though end element exists
        assert_eq!(find_geometry("a.a", &geometries, &geometry_names), None);
        assert_eq!(find_geometry("c.a", &geometries, &geometry_names), None);

        // parent missing in path
        assert_eq!(find_geometry("b1.b1a", &geometries, &geometry_names), None);
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
                <Break DMXBreak="2" DMXOffset="3"/>
                <Break DMXBreak="1" DMXOffset="2"/>
            </GeometryReference>
        </Geometry>
    </Geometries>
</FixtureType>
        "#;

        let (problems, geometries, _geometry_names) = run_parse_geometries(ft_str);

        assert!(problems.is_empty());
        assert_eq!(geometries.node_count(), 4)
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

        let (problems, geometries, _geometry_names) = run_parse_geometries(ft_str);

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::XmlAttributeMissing { .. }));

        assert_eq!(geometries.node_count(), 2);

        geometries
            .raw_nodes()
            .iter()
            .for_each(|n| assert!(n.weight.name() != ""));

        let name_of_broken_node = geometries.raw_nodes()[0].weight.name();
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

        let (problems, geometries, _geometry_names) = run_parse_geometries(ft_str);

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::DuplicateGeometryName(..)));

        let name_of_broken_node = geometries.raw_nodes()[3].weight.name();
        assert_is_uuid_and_not_nil(name_of_broken_node);
    }

    fn run_parse_geometries(
        ft_str: &str,
    ) -> (
        Vec<Problem>,
        Graph<GeometryType, ()>,
        HashMap<String, NodeIndex>,
    ) {
        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        let mut geometries = Graph::new();
        let mut geometry_names: HashMap<String, NodeIndex> = HashMap::new();
        parse_geometries(
            &mut geometries,
            &mut geometry_names,
            &ft,
            &mut problems,
            &doc,
        );
        (problems, geometries, geometry_names)
    }

    fn assert_is_uuid_and_not_nil(s: &str) {
        let uuid_pattern =
            Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
        assert!(uuid_pattern.is_match(s));
        let uuid_nil_pattern = Regex::new(r"00000000-0000-0000-0000-000000000000").unwrap();
        assert!(!uuid_nil_pattern.is_match(s));
    }
}
