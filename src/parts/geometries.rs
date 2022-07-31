use petgraph::{graph::{NodeIndex}, visit::EdgeRef, Directed, Graph};
use roxmltree::Node;

use crate::Problem;

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

impl GeometryType {
    fn name(&self) -> &str {
        match self {
            GeometryType::Root => "",
            GeometryType::Geometry { name } | GeometryType::Reference { name, .. } => name,
        }
    }
}

const GEOMETRY_TAGS: [&str; 18] = [
    "Geometry",
    "Axis",
    "FilterBeam",
    "FilterColor",
    "FilterGobo",
    "FilterShaper",
    "Beam",
    "MediaServerLayer",
    "MediaServerCamera",
    "MediaServerMaster",
    "Display",
    "GeometryReference",
    "Laser",
    "WiringObject",
    "Inventory",
    "Structure",
    "Support",
    "Magnet",
];

// TODO remove things that throw: todo!, unwrap, etc.

pub fn parse_geometries(geometries: &mut Geometries, ft: &Node, problems: &mut Vec<Problem>) {
    let g = match ft.children().find(|n| n.has_tag_name("Geometries")) {
        Some(g) => g,
        None => {
            problems.push(Problem::NodeMissing {
                missing: "Geometries".to_owned(),
                parent: "FixtureType".to_owned(),
            });
            return;
        }
    };

    let graph_root = geometries.add_node(GeometryType::Root);

    // First, add top-level geometries. These must exist so a GeometryReference
    // later on can be linked to a NodeIndex.
    g.children().filter(|n| n.is_element()).for_each(|n| {
        match n.tag_name().name() {
            tag @ ("Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo"
            | "FilterShaper" | "Beam" | "MediaServerLayer" | "MediaServerCamera"
            | "MediaServerMaster" | "Display" | "Laser" | "WiringObject" | "Inventory"
            | "Structure" | "Support" | "Magnet") => {
                let new_graph_node = geometries.add_node(GeometryType::Geometry {
                    name: n
                        .attribute("Name")
                        .unwrap_or_else(|| {
                            problems.push(Problem::AttributeMissing {
                                attr: "Name".to_owned(),
                                node: format! {"Geometries/{}", tag}, // TODO test this...
                            });
                            "" // TODO if the node has no name attr, maybe it should at least be given a unique identifier. Maybe "No Name {uuid}"?
                            // Without a name, it can't be referenced anyway
                        })
                        .to_owned(),
                });
                geometries.add_edge(graph_root, new_graph_node, ());
            }
            "GeometryReference" => todo!("GeometryReference not allowed at top level"),
            _ => todo!("Unknown Geometry type"),
        };
    });

    // Next, add non-top-level geometries.
    g.children().filter(|n| n.is_element()).for_each(|n| {
        let graph_index = geometries
            .edges(graph_root)
            .map(|edge| edge.target())
            // TODO matching an element based on a default name of "" is stupid. Is there no way we can know the associated XML node without searching for it like this?
            .find(|child_ind| geometries[*child_ind].name() == n.attribute("Name").unwrap_or("")) 
            .unwrap();
        add_children(&n, graph_index, geometries, problems);
    });

    // TODO we must validate that geometry names are unique, it's required in
    // the standard and the result would otherwise not be too useful since it
    // can't be re-serialized to a valid GDTF
    // maybe use a set of names?
    // what to do if a name is duplicate? Add Problem and change to "{duplicate name} {uuid}"?
}

fn add_children(
    parent_xml: &Node,
    parent_tree: NodeIndex,
    geometries: &mut Geometries,
    problems: &mut Vec<Problem>,
) {
    parent_xml
        .children()
        .filter(|n| n.is_element())
        .for_each(|n| {
            match n.tag_name().name() {
                tag @ ("Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo"
                | "FilterShaper" | "Beam" | "MediaServerLayer" | "MediaServerCamera"
                | "MediaServerMaster" | "Display" | "Laser" | "WiringObject"
                | "Inventory" | "Structure" | "Support" | "Magnet") => {
                    let ind = geometries.add_node(GeometryType::Geometry {
                        name: n
                            .attribute("Name")
                            .unwrap_or_else(|| {
                                problems.push(Problem::AttributeMissing {
                                    attr: "Name".to_owned(),
                                    node: format! {"Geometries//*[@'{}']/{}", geometries[parent_tree].name(), tag}, // TODO test this
                                });
                                ""
                            })
                            .to_owned(),
                    });
                    geometries.add_edge(parent_tree, ind, ());
                    add_children(&n, ind, geometries, problems);
                }
                tag @ "GeometryReference" => {
                    let ind = geometries.add_node(GeometryType::Reference {
                        name: n // TODO code duplication with other Geometry Types
                            .attribute("Name")
                            .unwrap_or_else(|| {
                                problems.push(Problem::AttributeMissing {
                                    attr: "Name".to_owned(),
                                    node: format! {"Geometries//*[@'{}']/{}", geometries[parent_tree].name(), tag}, // TODO test this
                                });
                                ""
                            })
                            .to_owned(),
                        reference: (),
                        break_offsets: (),
                    });
                    geometries.add_edge(parent_tree, ind, ());
                }
                tag => todo!("Unknown Geometry type tag {}", tag),
            };
        });
}
