use std::collections::hash_map::Entry::Vacant;

use petgraph::graph::NodeIndex;
use roxmltree::Node; // TODO import qualified, so it's easier to distinguish from petgraph::Node?

use super::errors::*;

use crate::{
    geometries::{Geometries, GeometryType, Offset, Offsets},
    Problems,
};

use super::utils::GetFromNode;

#[allow(dead_code)] // TODO remove once fields are used in deduplication
struct GeometryDuplicate<'a> {
    /// already parsed 'Name' attribute on xml_node, can't parse again due to side effects on get_name
    name: String,
    xml_node: Node<'a, 'a>,
    /// None if duplicate is top-level
    parent_graph_ind: Option<NodeIndex>,
}

pub fn parse_geometries(
    geometries: &mut Geometries,
    ft: &Node,
    problems: &mut Problems,
) -> Result<(), Error> {
    let g = match ft.find_child_by_tag_name("Geometries") {
        Ok(g) => g,
        Err(p) => {
            p.handled_by("leaving geometries empty", problems);
            return Ok(());
        }
    };

    let top_level_geometries = g.children().filter(|n| n.is_element());
    let mut parsed_top_level_geometries: Vec<(NodeIndex, Node)> = vec![];

    let mut geometry_duplicates = Vec::<GeometryDuplicate>::new();

    // First, add top-level geometries. These must exist so a GeometryReference
    // later on can be linked to a NodeIndex.
    for (i, n) in top_level_geometries.clone().enumerate() {
        match n.tag_name().name() {
            "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo" | "FilterShaper"
            | "Beam" | "MediaServerLayer" | "MediaServerCamera" | "MediaServerMaster"
            | "Display" | "Laser" | "WiringObject" | "Inventory" | "Structure" | "Support"
            | "Magnet" => {
                let name = match get_unique_geometry_name(n, i, geometries,
                     &mut geometry_duplicates, None, problems){
                        Some(name) => name,
                        None => continue,
                     };
                let geometry = GeometryType::Geometry { name };
                let graph_ind = geometries.add(geometry, None).ok_or(Error::Unexpected(
                    "Geometry Names must be unique once adding top level geometries".to_owned(),
                ))?;
                parsed_top_level_geometries.push((graph_ind, n));
            }
            "GeometryReference" => ProblemType::UnexpectedTopLevelGeometryReference(
                n.attribute("Name")
                    .unwrap_or("no `Name` attribute")
                    .to_owned(),
            )
            .at(&n)
            // TODO keep the problem message but handle by just parsing it, it's not actively harmful
            // so we should stick with the third principle: Suck out as much information as possible
            .handled_by("ignoring node because top-level GeometryReference could only be used for a DMX mode \
            that is offset from another one, which is useless because one can just change the start address in \
            the console", problems), 
            tag => ProblemType::UnexpectedXmlNode(tag.into()).at(&n).handled_by("ignoring node", problems),
        };
    }

    // Next, add non-top-level geometries.
    for (graph_ind, n) in parsed_top_level_geometries.iter() {
        add_children(
            *n,
            *graph_ind,
            geometries,
            problems,
            &mut geometry_duplicates,
        )?;
    }

    // TODO refactor to function for more semantic naming?
    // handle geometry duplicates after all others, to ensure deduplicated names don't conflict with defined names
    while let Some(dup) = geometry_duplicates.pop() {
        let mut suggested_name = dup.name.clone();
        let mut goes_into_lookup = false;

        // check if duplicate and original are in different top level geometry, if yes, suggest semantic renaming
        if let Some(duplicate_parent) = dup.parent_graph_ind {
            let original_ind = geometries.find(&dup.name).ok_or_else(|| {
                Error::Unexpected("Geometry Duplicate Name not found anymore".into())
            })?;

            let original_top_level = geometries.top_level_geometry(original_ind);
            let duplicate_top_level = geometries.top_level_geometry(duplicate_parent);

            if original_top_level != duplicate_top_level {
                let duplicate_top_level_name = geometries.graph[duplicate_top_level].name();
                suggested_name = format!("{} (in {})", dup.name, duplicate_top_level_name);
                goes_into_lookup = true;
                if !geometries.names.contains_key(&suggested_name) {
                    todo!("add to geometries with suggested name, pass in `goes_into_lookup` and act accordingly, push problem about duplicate geometry, then add_children");
                    continue;
                }
            }
        }

        let mut dedup_ind: u16 = 1;

        loop {
            suggested_name = format!("{} (duplicate {})", suggested_name, dedup_ind);
            if !geometries.names.contains_key(&suggested_name) {
                todo!("add to geometries with suggested name, pass in `goes_into_lookup` and act accordingly, push problem about duplicate geometry, then add_children");
                break;
            }
            dedup_ind = match dedup_ind.checked_add(1) {
                Some(v) => v,
                None => {
                    ProblemType::DuplicateGeometryName(dup.name)
                        .at(&dup.xml_node)
                        .handled_by("deduplication failed, ignoring node", problems);
                    break;
                }
            };
        }
    }

    Ok(())
}

// TODO add functions for "adding a geometry node" with a custom name, behavior also depends on whether the node is top-level

fn get_unique_geometry_name<'a>(
    n: Node<'a, 'a>,
    node_index_in_xml_parent: usize,
    geometries: &Geometries,
    geometry_duplicates: &mut Vec<GeometryDuplicate<'a>>,
    parent_graph_ind: Option<NodeIndex>,
    problems: &mut Problems,
) -> Option<String> {
    let name = n.get_name(node_index_in_xml_parent, problems);
    match geometries.names.get(&name) {
        None => Some(name),
        Some(_duplicate_graph_ind) => {
            geometry_duplicates.push(GeometryDuplicate {
                xml_node: n,
                parent_graph_ind,
                name,
            });
            None
        }
    }
}

/// Recursively adds all children geometries of a parent to the geometries struct
fn add_children<'a>(
    parent_xml: Node<'a, 'a>,
    parent_tree: NodeIndex,
    geometries: &mut Geometries,
    problems: &mut Problems,
    geometry_duplicates: &mut Vec<GeometryDuplicate<'a>>,
) -> Result<(), Error> {
    let children = parent_xml.children().filter(|n| n.is_element());

    for (i, n) in children.enumerate() {
        match n.tag_name().name() {
            "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo" | "FilterShaper"
            | "Beam" | "MediaServerLayer" | "MediaServerCamera" | "MediaServerMaster"
            | "Display" | "Laser" | "WiringObject" | "Inventory" | "Structure" | "Support"
            | "Magnet" => {
                let name = match get_unique_geometry_name(
                    n,
                    i,
                    geometries,
                    geometry_duplicates,
                    Some(parent_tree),
                    problems,
                ) {
                    Some(name) => name,
                    None => continue,
                };
                let geometry = GeometryType::Geometry { name };
                let i = geometries
                    .add(geometry, Some(parent_tree))
                    .ok_or(Error::Unexpected(
                        "Geometry Names must be unique when adding geometries".to_owned(),
                    ))?;
                add_children(n, i, geometries, problems, geometry_duplicates)?;
            }
            "GeometryReference" => parse_geometry_reference(
                n,
                i,
                geometries,
                geometry_duplicates,
                Some(parent_tree),
                problems,
            )?,
            tag => ProblemType::UnexpectedXmlNode(tag.into())
                .at(&n)
                .handled_by("ignoring node", problems),
        };
    }
    Ok(())
}

fn parse_geometry_reference<'a>(
    n: Node<'a, 'a>,
    node_index_in_xml_parent: usize,
    geometries: &mut Geometries,
    geometry_duplicates: &mut Vec<GeometryDuplicate<'a>>,
    parent_graph_ind: Option<NodeIndex>,
    problems: &mut Problems,
) -> Result<(), Error> {
    let name = match get_unique_geometry_name(
        n,
        node_index_in_xml_parent,
        geometries,
        geometry_duplicates,
        parent_graph_ind,
        problems,
    ) {
        Some(name) => name,
        None => return Ok(()),
    };
    let ref_ind = match get_index_of_referenced_geometry(n, geometries, &name) {
        Ok(i) => i,
        Err(p) => {
            p.handled_by("ignoring node", problems);
            return Ok(());
        }
    };
    let offsets = parse_reference_offsets(&n, &name, problems);

    let geometry = GeometryType::Reference {
        name,
        offsets,
        reference: ref_ind,
    };
    geometries
        .add(geometry, parent_graph_ind)
        .ok_or(Error::Unexpected(
            "Geometry Names must be unique when adding geometry references".to_owned(),
        ))?;
    Ok(())
}

// TODO fix warning later, it is only a memory usage problem, due to an enum
// variant in `ProblemType` with many fields
#[allow(clippy::result_large_err)]
fn get_index_of_referenced_geometry(
    n: Node,
    geometries: &mut Geometries,
    name: &str,
) -> Result<NodeIndex, Problem> {
    let ref_string = n.parse_required_attribute::<String>("Geometry")?;
    let ref_ind = geometries
        .find(&ref_string)
        .ok_or_else(|| ProblemType::UnknownGeometry("ref_string".into()).at(&n))?;
    if geometries.is_top_level(ref_ind) {
        Ok(ref_ind)
    } else {
        Err(ProblemType::NonTopLevelGeometryReferenced {
            target: ref_string,
            geometry_reference: name.into(),
        }
        .at(&n))
    }
}

fn parse_reference_offsets(&n: &Node, n_name: &str, problems: &mut Problems) -> Offsets {
    let mut nodes = n
        .children()
        .filter(|n| n.tag_name().name() == "Break")
        .rev(); // start at last element, which provides the Overwrite offset if present

    let mut offsets = Offsets::new();

    nodes.next().and_then(|last_break| {
        let dmx_break = last_break
            .parse_required_attribute("DMXBreak")
            .handled_by("ignoring node", problems)?;
        let offset = last_break
            .parse_required_attribute("DMXOffset")
            .handled_by("ignoring node", problems)?;
        offsets.overwrite = Some(Offset { dmx_break, offset });
        Some(())
    });

    for element in nodes {
        if let (Some(dmx_break), Some(offset)) = (
            element
                .parse_required_attribute("DMXBreak")
                .handled_by("ignoring node", problems),
            element
                .parse_required_attribute("DMXOffset")
                .handled_by("ignoring node", problems),
        ) {
            if offsets.normal.contains_key(&dmx_break) {
                ProblemType::DuplicateDmxBreak {
                    duplicate_break: dmx_break,
                    geometry_reference_name: n_name.into(),
                }
                .at(&n)
                .handled_by("overwriting previous value", problems)
            }
            offsets.normal.insert(dmx_break, offset);
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

// allow unwrap/expect eplicitly, because clippy.toml config doesn't work properly yet
// fixed in https://github.com/rust-lang/rust-clippy/pull/9686
// TODO remove once Clippy 0.1.67 is available
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Not;

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
            let (mut problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 1);
            assert!(matches!(
                problems.pop().unwrap().problem_type(),
                ProblemType::DuplicateDmxBreak {
                    duplicate_break: 2,
                    geometry_reference_name
                }
            ));
            assert_eq!(offsets.normal[&2], 2); // higher element takes precedence
        }

        #[test]
        fn empty_reference_offsets() {
            let xml = r#"<GeometryReference />"#;
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 0);
            assert_eq!(offsets, Offsets::new());
        }

        fn run_parse_reference_offsets(xml: &str) -> (Problems, Offsets) {
            let doc = roxmltree::Document::parse(xml).unwrap();
            let n = doc.root_element();
            let mut problems: Problems = vec![];
            let offsets = parse_reference_offsets(&n, "arbitrary name for testing", &mut problems);
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
    /// Geometries should really have a Name attribute, but according to GDTF
    /// 1.2 (DIN SPEC 15800:2022-02), the default value for "Name" type values
    /// is: "object type with an index in parent".  
    /// I think the reasonable choice is to still report these problems, but
    /// report which default name was assigned. If there are name duplicates in
    /// default names, they can be handled and report like any other.
    fn geometry_names_missing() {
        let (problems, geometries) = run_parse_geometries(
            r#"
            <FixtureType>
                <Geometries>
                    <Beam /> <!--default name "Beam 1"-->
                    <Geometry /> <!--default name "Geometry 2"-->
                    <Geometry> <!--default name "Geometry 3"-->
                        <Geometry /> <!--default name "Geometry 1"-->
                        <Geometry /> <!--default name "Geometry 2", deduplicated to "Geometry 2 (in Geometry 3)"-->
                        <GeometryReference Geometry="Geometry 2"/> <!--default name "GeometryReference 3"-->
                    </Geometry>
                </Geometries>
            </FixtureType>
            "#,
        );

        for p in problems[0..6].iter() {
            assert!(
                matches!(p.problem_type(), ProblemType::XmlAttributeMissing { attr, .. } if attr == "Name")
            );
        }

        assert!(matches!(
            problems[6].problem_type(),
            ProblemType::DuplicateGeometryName(dup) if dup == "Geometry 2"
        ));

        let b = geometries.find("Beam 1").unwrap();
        assert!(geometries.is_top_level(b) && geometries.count_children(b) == 0);

        let g2 = geometries.find("Geometry 2").unwrap();
        assert!(geometries.is_top_level(g2) && geometries.count_children(g2) == 0);

        let g3 = geometries.find("Geometry 3").unwrap();
        assert!(geometries.is_top_level(g3));

        let mut g3_children = geometries
            .graph
            .neighbors(g3)
            .map(|ind| &geometries.graph[ind]);

        assert!(matches!(
            g3_children.find(|g| g.name() == "Geometry 1").unwrap(),
            GeometryType::Geometry { .. }
        ));
        assert!(matches!(
            g3_children
                .find(|g| g.name() == "Geometry 2 (in Geometry 3)")
                .unwrap(), // TODO broken until deterministic renaming is implemented
            GeometryType::Geometry { .. }
        ));
        assert!(matches!(
            g3_children
                .find(|g| g.name() == "GeometryReference 3")
                .unwrap(),
            GeometryType::Reference { .. }
        ));

        assert_eq!(3, g3_children.count());
    }

    #[test]
    fn geometry_duplicate_names() {
        // TODO add a third level in top-level geometry that is deduplicated. Those geometries should not be added multiple times
        let ft_str = r#"
        <FixtureType>
            <Geometries>
                <Geometry Name="Top 1">                    
                    <Geometry Name="Element 1"/>
                    <Geometry Name="Element 1"/> <!-- 2) rename to "Element 1 (duplicate 1)" -->
                    <Geometry Name="Top 2"/> <!-- 3) rename to "Top 2 (in Top 1)" -->
                </Geometry>
                <Geometry Name="Top 2"/>
                <Geometry Name="Top 2"> <!-- 1) rename to "Top 2 (duplicate 1)" -->
                    <Geometry Name="Element 1"/> <!-- 5) rename to "Element 1 (in Top 2) (duplicate 1)" -->
                    <Geometry Name="Element 1 (in Top 2)"/>
                    <Geometry Name="Top 2"/> <!-- 6) rename to "Top 2 (in Top 2)" -->
                </Geometry>
                <Geometry Name="Top 3">
                    <GeometryReference Geometry="Top 2" Name="Element 1"> 
                    <!-- 4) rename to "Element 1 (in Top 3)", should reference the one w/o children -->
                        <Break DMXBreak="1" DMXOffset="1"/>
                    </GeometryReference>
                </Geometry>
            </Geometries>
        </FixtureType>
                "#;

        let (problems, geometries) = run_parse_geometries(ft_str);

        assert_eq!(problems.len(), 6);
        for p in problems.iter() {
            assert!(matches!(
                p.problem_type(),
                ProblemType::DuplicateGeometryName(..)
            ))
        }

        let t1 = geometries.find("Top 1").unwrap();
        assert!(geometries.is_top_level(t1));
        let mut t1_children = geometries.children(t1);
        t1_children.find(|g| g.name() == "Element 1").unwrap();
        t1_children
            .find(|g| g.name() == "Element 1 (duplicate 1)")
            .unwrap();
        t1_children
            .find(|g| g.name() == "Top 2 (in Top 1)")
            .unwrap();
        assert_eq!(t1_children.count(), 3);

        let t2 = geometries.find("Top 2").unwrap();
        assert!(geometries.is_top_level(t2));
        assert_eq!(geometries.count_children(t2), 0);

        let t2d = geometries.find("Top 2 (duplicate 1)").unwrap();
        assert!(geometries.is_top_level(t2d));
        let mut t2d_children = geometries.children(t2d);
        t2d_children
            .find(|g| g.name() == "Element 1 (in Top 2) (duplicate 1)")
            .unwrap();
        t2d_children
            .find(|g| g.name() == "Element 1 (in Top 2)")
            .unwrap();
        t2d_children
            .find(|g| g.name() == "Top 2 (in Top 2)")
            .unwrap();
        assert_eq!(t2d_children.count(), 3);

        let t3 = geometries.find("Top 3").unwrap();
        assert!(geometries.is_top_level(t3));
        let mut t3_children = geometries.children(t3);
        let reference = t3_children
            .find(|g| g.name() == "Element 1 (in Top 3)")
            .unwrap();
        assert_eq!(t3_children.count(), 1);
        assert!(matches!(reference, GeometryType::Reference { reference, .. } if *reference == t2));
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
            problems[0].problem_type(),
            ProblemType::NonTopLevelGeometryReferenced { .. },
        ));

        assert_eq!(geometries.graph.node_count(), 3);
        assert!(geometries.names.contains_key("Element 1").not());
    }

    fn run_parse_geometries(ft_str: &str) -> (Problems, Geometries) {
        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Problems = vec![];
        let mut geometries = Geometries::default();
        parse_geometries(&mut geometries, &ft, &mut problems).unwrap();
        (problems, geometries)
    }
}
