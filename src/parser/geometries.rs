use std::collections::hash_map::Entry::Vacant;

use petgraph::graph::NodeIndex;
use roxmltree::Node; // TODO import qualified, so it's easier to distinguish from petgraph::Node

use super::errors::*;

use crate::geometries::{Geometries, GeometryType, Offset, Offsets};

use super::utils::GetFromNode;

struct GeometryDuplicate<'a> {
    /// already parsed 'Name' attribute
    // duplicate_name: String, // TODO just get it from the problem
    xml_node: Node<'a, 'a>,
    /// None if duplicate is top-level
    parent_graph_ind: Option<NodeIndex>,
    problem: Problem,
}

pub fn parse_geometries(
    geometries: &mut Geometries,
    ft: &Node,
    problems: &mut Vec<HandledProblem>,
) -> Result<(), Error> {
    let g = match ft.find_child_by_tag_name("Geometries") {
        Ok(g) => g,
        Err(p) => {
            p.handled_by("leaving geometries empty", problems);
            return Ok(());
        }
    };

    let top_level_geometries = g.children().filter(|n| n.is_element());
    let mut top_level_geometry_graph_indices: Vec<NodeIndex> = vec![];

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
                let i = geometries.add(geometry, None).ok_or(Error::Unexpected(
                    "Geometry Names must be unique once adding top level geometries".to_owned(),
                ))?;
                top_level_geometry_graph_indices.push(i);
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
    for (i, n) in top_level_geometries.enumerate() {
        let graph_index = top_level_geometry_graph_indices[i];
        add_children(
            n,
            graph_index,
            geometries,
            problems,
            &mut geometry_duplicates,
        )?;
    }

    // TODO handle geometries deduplication
    // handle geometry duplicates after all others, to ensure deduplicated names don't conflict with other names
    while !geometry_duplicates.is_empty() {
        let dup = geometry_duplicates
            .pop()
            .ok_or_else(|| Error::Unexpected("geometry duplicates empty".into()))?;

        // TODO handle properly according to this algorithm:
        // - if the original to the duplicate is in a different top-level geometry, append `(in top-level geometry)`,
        //   and continue with further duplications checks (it could still conflict with an existing). Once the name is final,
        //   put them into a look-up table with top-level geometry and their old name, so that when they are linked the new name can be found that way
        // - otherwise (or if still duplicated), append `(duplicate i)` and increase i until there are no duplicates
        //   a look up table is not necessary in that case, as these geometries can never be referenced on their own

        // TODO check what happens if the name of a non-top-level geometry is
        // the same as a later occuring top-level geometry that is referenced by
        // a <GeometryReference>. It should work fine.

        // quick and dirty
        dup.problem.handled_by("ignoring node", problems);
    }

    Ok(())
}

fn get_unique_geometry_name<'a>(
    n: Node<'a, 'a>,
    node_index_in_xml_parent: usize,
    geometries: &Geometries,
    geometry_duplicates: &mut Vec<GeometryDuplicate<'a>>,
    parent_graph_ind: Option<NodeIndex>,
    problems: &mut Vec<HandledProblem>,
) -> Option<String> {
    let name = n.get_name(node_index_in_xml_parent, problems);
    match geometries.names.get(&name) {
        None => Some(name),
        Some(_duplicate_graph_ind) => {
            geometry_duplicates.push(GeometryDuplicate {
                xml_node: n,
                parent_graph_ind,
                problem: ProblemType::DuplicateGeometryName(name).at(&n),
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
    problems: &mut Vec<HandledProblem>,
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
    problems: &mut Vec<HandledProblem>,
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

fn parse_reference_offsets(&n: &Node, n_name: &str, problems: &mut Vec<HandledProblem>) -> Offsets {
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
    use std::ops::Not;

    use petgraph::{visit::IntoEdgesDirected, Direction::Outgoing};
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

        todo!("set up problem type to support actions for all problems");

        // assert!(
        //     matches!(problems[0], Problem::XmlAttributeMissing { attr, action .. } if attr == "Name" && action.contains("Beam 1"))
        // );
        // assert!(
        //     matches!(problems[1], Problem::XmlAttributeMissing { attr, action .. } if attr == "Name" && action.contains("Geometry 2"))
        // );
        // assert!(
        //     matches!(problems[2], Problem::XmlAttributeMissing { attr, action .. } if attr == "Name" && action.contains("Geometry 3"))
        // );
        // assert!(
        //     matches!(problems[3], Problem::XmlAttributeMissing { attr, action .. } if attr == "Name" && action.contains("Geometry 1"))
        // );
        // assert!(
        //     matches!(problems[4], Problem::XmlAttributeMissing { attr, action .. } if attr == "Name" && action.contains("Geometry 2"))
        // );
        // assert!(
        //     matches!(problems[5], Problem::XmlAttributeMissing { attr, action .. } if attr == "Name" && action.contains("GeometryReference 3"))
        // );
        // assert!(
        //     matches!(problems[6], Problem::DuplicateGeometryName(dup, _, dedup) if dup == "Geometry 2" && dedup == "Geometry 2 (in Geometry 3)")
        // );

        // let beam_ind = geometries.find("Beam 1").unwrap();
        // assert!(geometries
        //     .graph
        //     .neighbors_undirected(beam_ind)
        //     .next()
        //     .is_none());

        // let g2_ind = geometries.find("Geometry 2").unwrap();
        // assert!(geometries
        //     .graph
        //     .neighbors_undirected(g2_ind)
        //     .next()
        //     .is_none());

        // let g3_ind = geometries.find("Geometry 3").unwrap();
        // assert!(geometries.is_top_level(g3_ind));
        // let g3_children = geometries.graph.neighbors(g3_ind);
        // let children = g3_children.map(|ind| geometries.graph[ind]).collect();
        // assert_eq(3, children.len());

        todo!("assert that the names and types of the children are as expected")
        // children.any(|g| matches!(g, GeometryType));
    }

    #[test]
    fn geometry_duplicate_name_deterministic_renaming() {
        todo!()
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
        parse_geometries(&mut geometries, &ft, &mut problems).unwrap();
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
