use petgraph::{graph::NodeIndex, Directed, Graph};
use roxmltree::{Document, Node};
use uuid::Uuid;

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
                let new_graph_node = geometries.add_node(GeometryType::Geometry {
                    name: geometry_name(n, problems, doc), // TODO test this error handling
                });
                geometries.add_edge(graph_root, new_graph_node, ());
                top_level_geometry_graph_indices.push(new_graph_node);
            }
            "GeometryReference" => todo!("GeometryReference not allowed at top level"),
            _ => todo!("Unknown Geometry type"),
        };
    });

    // Next, add non-top-level geometries.
    top_level_geometries.iter().enumerate().for_each(|(i, n)| {
        let graph_index = top_level_geometry_graph_indices[i];
        add_children(n, graph_index, geometries, problems, doc);
    });

    // TODO we must validate that geometry names are unique, it's required in
    // the standard and the result would otherwise not be too useful since it
    // can't be re-serialized to a valid GDTF
    // maybe use a set of names?
    // what to do if a name is duplicate? Add Problem and change to "{duplicate name} {uuid}"?
}

fn geometry_name(n: &Node, problems: &mut Vec<Problem>, doc: &Document) -> String {
    get_string_attribute(n, "Name", problems, doc)
        .unwrap_or_else(|| format!("No Name {}", Uuid::new_v4()))
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
                    let ind = geometries.add_node(GeometryType::Geometry {
                        name: geometry_name(&n, problems, doc), // TODO test this error handling
                    });
                    geometries.add_edge(parent_tree, ind, ());
                    add_children(&n, ind, geometries, problems, doc);
                }
                "GeometryReference" => {
                    let ind = geometries.add_node(GeometryType::Reference {
                        name: geometry_name(&n, problems, doc), // TODO test this error handling
                        reference: (),
                        break_offsets: (),
                    });
                    geometries.add_edge(parent_tree, ind, ());
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

        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        let mut geometries = Graph::new();
        parse_geometries(&mut geometries, &ft, &mut problems, &doc);

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

        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Vec<Problem> = vec![];
        let mut geometries = Graph::new();
        parse_geometries(&mut geometries, &ft, &mut problems, &doc);

        println!("{}", problems[0]);
        println!("{:#?}", geometries); // TODO remove prints

        assert_eq!(problems.len(), 1);

        assert_eq!(geometries.node_count(), 5);

        geometries
            .raw_nodes()
            .iter()
            .for_each(|n| assert!(n.weight.name().unwrap_or("root") != ""));

        let name_of_broken_node = geometries.raw_nodes()[1].weight.name().unwrap();

        // Name of broken node contains description and a UUID
        assert!(name_of_broken_node.contains("No Name"));
        let uuid_pattern = Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
        assert!(uuid_pattern.is_match(name_of_broken_node));
        let uuid_nil_pattern = Regex::new(r"00000000-0000-0000-0000-000000000000").unwrap();
        assert!(!uuid_nil_pattern.is_match(name_of_broken_node));
    }
}
