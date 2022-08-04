use std::collections::HashMap;

use petgraph::{graph::NodeIndex, Directed, Graph};
use roxmltree::{Document, Node};
use uuid::Uuid;

use crate::node_position;
use crate::{get_string_attribute, Problem};

/// Graph representing the Geometry tree.
///
/// Edges point from parent to child.
pub type Geometries = Graph<GeometryType, (), Directed>;

#[derive(Debug)]
pub enum GeometryType {
    Root,
    Geometry {
        name: String,
    },
    Reference {
        name: String,
        reference: (),
        break_offsets: (),
    },
}

// TODO remove things that throw: todo!, unwrap, etc.

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
            });
            return;
        }
    };

    let graph_root = geometries.add_node(GeometryType::Root);

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
                geometries.add_edge(graph_root, new_graph_node, ());
                top_level_geometry_graph_indices.push(new_graph_node);
                geometry_names.insert(name, new_graph_node);
            }
            "GeometryReference" => todo!("GeometryReference not allowed at top level"),
            _ => todo!("Unknown Geometry type"),
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
                    let ind = geometries.add_node(GeometryType::Reference {
                        name: name.to_owned(),
                        reference: (),
                        break_offsets: (),
                    });
                    geometries.add_edge(parent_tree, ind, ());
                    geometry_names.insert(name, ind);
                }
                tag => todo!("Unknown Geometry type tag {}", tag),
            };
        });
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::*;

    impl GeometryType {
        fn name(&self) -> Option<&str> {
            match self {
                GeometryType::Root => None,
                GeometryType::Geometry { name } | GeometryType::Reference { name, .. } => {
                    Some(name)
                }
            }
        }
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
        assert_eq!(geometries.node_count(), 5)
    }

    #[test]
    fn geometries_top_level_name_missing() {
        let ft_str = r#"
<FixtureType>
    <Geometries>
        <Geometry Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
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

        assert_eq!(problems.len(), 1);
        assert!(matches!(problems[0], Problem::XmlAttributeMissing { .. }));

        assert_eq!(geometries.node_count(), 5);

        geometries
            .raw_nodes()
            .iter()
            .for_each(|n| assert!(n.weight.name().unwrap_or("root") != ""));

        let name_of_broken_node = geometries.raw_nodes()[1].weight.name().unwrap();
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

        let name_of_broken_node = geometries.raw_nodes()[4].weight.name().unwrap();
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
