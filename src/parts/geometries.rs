use indextree::{Arena, NodeId};
use roxmltree::Node;

use crate::{errors::Problem};

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

pub fn parse_geometries(
    geometries: &mut Arena<GeometryType>,
    ft: &Node,
    problems: &mut Vec<Problem>,
) {
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

    let root = geometries.new_node(GeometryType::Root);

    add_nodes(&g, &root, geometries, problems);
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

// TODO remove unwraps
fn add_nodes(
    parent_xml_node: &Node,
    parent_tree_node: &NodeId,
    geometries: &mut Arena<GeometryType>,
    problems: &mut Vec<Problem>,
) {
    println!("starting XML node {:#?}", parent_xml_node);
    parent_xml_node
    // TODO  won't work  reliably because it's depth-first and first level needs to be done first so that referenced nodes are ready
        .descendants()
        .skip(1)  // skip parent itself
        .filter(|n| n.is_element() && GEOMETRY_TAGS.contains(&n.tag_name().name()))
        .for_each(|xml_child| {
            let name = xml_child
                .attribute("Name")
                .unwrap_or_else(|| {
                    println! {"no Name attribute{:#?}", xml_child};
                    ""
                })
                .to_owned(); // TODO if it's a DMXBreak, it won't have a name
            let geometry = match xml_child.tag_name().name() {
                "GeometryReference" => GeometryType::Reference {
                    name,
                    reference: (),
                    break_offsets: (),
                },
                _ => GeometryType::Geometry { name },
            };
            let new = geometries.new_node(geometry);
            parent_tree_node
                .checked_append(new, geometries)
                .unwrap_or_else(|e| problems.push(Problem::GeometryTreeError(e.to_string())));
        });
}
