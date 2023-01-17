use std::collections::{hash_map::Entry::Vacant, HashMap};

use petgraph::graph::NodeIndex;
use roxmltree::Node;

use super::errors::*;

use crate::{
    geometries::Geometries,
    geometry::{Geometry, Offset, Offsets, Type},
    types::name::{Name, NameError},
    Problems,
};

use super::utils::GetFromNode;

struct GeometryDuplicate<'a> {
    /// already parsed 'Name' attribute on xml_node, can't parse again due to side effects on get_name
    name: Name,
    xml_node: Node<'a, 'a>,
    /// None if duplicate is top-level
    parent_graph_ind: Option<NodeIndex>,
    duplicate_graph_ind: NodeIndex,
}

// TODO create a struct `GeometriesParser` or similar, then implement all these
// functions on that. That reduces the amount of argument we have to pass around
// and allows the functions to share some common state :). It is somewhat more
// Java-like though ("everything's a class").
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

    let mut gcx = GeometryParsingContext {
        geometries,
        geometry_duplicates: &mut geometry_duplicates,
        problems,
    };

    // First, add top-level geometries. These must exist so a GeometryReference
    // later on can be linked to a NodeIndex.
    for (i, n) in top_level_geometries.clone().enumerate() {
        if let Ok(graph_ind) = parse_element(i, n, None, &mut gcx) {
            parsed_top_level_geometries.push((graph_ind, n));
            if let Geometry {
                name,
                t: Type::Reference { .. },
            } = &gcx.geometries.graph()[graph_ind]
            {
                ProblemType::UnexpectedTopLevelGeometryReference(name.to_owned()).at(&n).handled_by("keeping GeometryReference, \
                but it is useless because a top-level GeometryReference can only be used for a DMX mode \
                that is offset from another one, which is useless because one can just change the start address in \
                the console", gcx.problems);
            }
        };
    }

    // Next, add non-top-level geometries.
    for (graph_ind, n) in parsed_top_level_geometries.iter() {
        add_children(*n, *graph_ind, &mut gcx);
    }

    // TODO factor out to its own struct with methods for lookup and adding elements
    let mut rename_lookup = GeometryRenameLookup::new();

    // handle geometry duplicates after all others, to ensure deduplicated names don't conflict with defined names
    while let Some(dup) = gcx.geometry_duplicates.pop() {
        let mut suggested_name: Name = dup.name.clone();

        // check if duplicate and original are in different top level geometry, if yes, suggest semantic renaming
        let duplicate_top_level = if let Some(duplicate_parent) = dup.parent_graph_ind {
            let original_top_level = gcx
                .geometries
                .top_level_geometry_index(dup.duplicate_graph_ind);
            let duplicate_top_level = gcx.geometries.top_level_geometry_index(duplicate_parent);

            if original_top_level != duplicate_top_level {
                let duplicate_top_level_name = &gcx.geometries.graph()[duplicate_top_level].name;
                suggested_name = format!("{} (in {})", dup.name, duplicate_top_level_name)
                    .try_into()
                    .unwrap_or_else(|e: NameError| e.name); // safe, because added chars are valid
                if !gcx.geometries.names().contains_key(&suggested_name) {
                    handle_renamed_geometry(
                        &dup,
                        &suggested_name,
                        &mut gcx,
                        &mut rename_lookup,
                        Some(duplicate_top_level),
                    );
                    continue;
                }
            }
            Some(duplicate_top_level)
        } else {
            None
        };

        // increment index until unique name is found
        let mut dedup_ind: u16 = 1;
        loop {
            // TODO, does this actually count up, or just appends `(duplicate i)` EVERY SINGLE TIME?
            suggested_name = format!("{} (duplicate {})", suggested_name, dedup_ind)
                .try_into()
                .unwrap_or_else(|e: NameError| e.name); // safe, because added chars are valid;
            if !gcx.geometries.names().contains_key(&suggested_name) {
                handle_renamed_geometry(
                    &dup,
                    &suggested_name,
                    &mut gcx,
                    &mut rename_lookup,
                    duplicate_top_level,
                );
                break;
            }
            dedup_ind = match dedup_ind.checked_add(1) {
                Some(v) => v,
                None => {
                    ProblemType::DuplicateGeometryName(dup.name)
                        .at(&dup.xml_node)
                        .handled_by("deduplication failed, ignoring node", gcx.problems);
                    break;
                }
            };
        }
    }

    // TODO return rename_lookup for later use when parsing modes

    Ok(())
}

/// (top level name, duplicate geometry name) => renamed name
type GeometryRenameLookup = HashMap<(Name, Name), Name>;

fn handle_renamed_geometry<'a>(
    dup: &GeometryDuplicate<'a>,
    suggested_name: &Name,
    gcx: &mut GeometryParsingContext<'a>,
    rename_lookup: &mut GeometryRenameLookup,
    duplicate_top_level: Option<NodeIndex>,
) {
    ProblemType::DuplicateGeometryName(dup.name.clone())
        .at(&dup.xml_node)
        .handled_by(format!("renamed to {}", suggested_name), gcx.problems);
    if let Ok(graph_ind) = parse_named_element(
        dup.xml_node,
        suggested_name.clone(),
        dup.parent_graph_ind,
        gcx,
    ) {
        if let Some(duplicate_top_level) = duplicate_top_level {
            rename_lookup.insert(
                (
                    // TODO index may panic, replace with geometries.graph().node_weight(ind)
                    gcx.geometries.graph()[duplicate_top_level].name.to_owned(),
                    dup.name.clone(),
                ),
                suggested_name.clone(),
            );
        };
        add_children(dup.xml_node, graph_ind, gcx);
    }
}

struct GeometryParsingContext<'a> {
    geometries: &'a mut Geometries,
    geometry_duplicates: &'a mut Vec<GeometryDuplicate<'a>>,
    problems: &'a mut Problems,
}

/// Parse the geometry element. If the result is Ok(graph_ind), the geometry was
/// added into the geometry graph at graph_ind and the caller should continue to
/// parse its children (if they exist). If the result is Err(()), an error was
/// handled otherwise and the caller should not continue to parse the children
/// on the node.
fn parse_element<'a>(
    node_index_in_xml_parent: usize,
    n: Node<'a, 'a>,
    parent_graph_ind: Option<NodeIndex>,
    gcx: &mut GeometryParsingContext<'a>,
) -> Result<NodeIndex, ()> {
    let name =
        get_unique_geometry_name(n, node_index_in_xml_parent, parent_graph_ind, gcx).ok_or(())?;

    parse_named_element(n, name, parent_graph_ind, gcx)
}

/// Parse the geometry element with the give name. If the
/// result is Ok(graph_ind), the geometry was added into the geometry graph at
/// graph_ind and the caller should continue to parse its children (if they
/// exist). If the result is Err(()), an error was handled otherwise and the
/// caller should not continue to parse the children on the node.
fn parse_named_element<'a>(
    n: Node<'a, 'a>,
    name: Name,
    parent_graph_ind: Option<NodeIndex>,
    gcx: &mut GeometryParsingContext<'a>,
) -> Result<NodeIndex, ()> {
    match n.tag_name().name() {
        "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo" | "FilterShaper"
        | "Beam" | "MediaServerLayer" | "MediaServerCamera" | "MediaServerMaster" | "Display"
        | "Laser" | "WiringObject" | "Inventory" | "Structure" | "Support" | "Magnet" => {
            let geometry = Geometry {
                name,
                t: Type::General,
            };
            add_to_geometries(geometry, parent_graph_ind, n, gcx)
        }
        "GeometryReference" => {
            let geometry = parse_geometry_reference(n, name, gcx)?;
            add_to_geometries(geometry, parent_graph_ind, n, gcx).and(Err(()))
            // don't parse children of GeometryReference as geometries
        }
        tag => {
            ProblemType::UnexpectedXmlNode(tag.into())
                .at(&n)
                .handled_by("ignoring node", gcx.problems);
            Err(())
        }
    }
}

fn add_to_geometries(
    geometry: Geometry,
    parent_graph_ind: Option<NodeIndex>,
    n: Node,
    gcx: &mut GeometryParsingContext,
) -> Result<NodeIndex, ()> {
    let graph_ind = match parent_graph_ind {
        Some(parent_graph_ind) => gcx.geometries.add(geometry, parent_graph_ind),
        None => gcx.geometries.add_top_level(geometry),
    }
    .map_err(|err| {
        ProblemType::Unexpected(err.to_string())
            .at(&n)
            .handled_by("ignoring node", gcx.problems)
    })?;
    Ok(graph_ind)
}

fn get_unique_geometry_name<'a>(
    n: Node<'a, 'a>,
    node_index_in_xml_parent: usize,
    parent_graph_ind: Option<NodeIndex>,
    gcx: &mut GeometryParsingContext<'a>,
) -> Option<Name> {
    let name = n.get_name(node_index_in_xml_parent, gcx.problems);
    match gcx.geometries.names().get(&name) {
        None => Some(name),
        Some(duplicate_graph_ind) => {
            gcx.geometry_duplicates.push(GeometryDuplicate {
                xml_node: n,
                parent_graph_ind,
                name,
                duplicate_graph_ind: *duplicate_graph_ind,
            });
            None
        }
    }
}

/// Recursively adds all children geometries of a parent to the geometries struct
fn add_children<'a>(
    parent_xml: Node<'a, 'a>,
    parent_tree: NodeIndex,
    gcx: &mut GeometryParsingContext<'a>,
) {
    let children = parent_xml.children().filter(|n| n.is_element());

    for (i, n) in children.enumerate() {
        if let Ok(graph_ind) = parse_element(i, n, Some(parent_tree), gcx) {
            add_children(n, graph_ind, gcx)
        }
    }
}

fn parse_geometry_reference<'a>(
    n: Node<'a, 'a>,
    name: Name,
    gcx: &mut GeometryParsingContext<'a>,
) -> Result<Geometry, ()> {
    let ref_ind = match get_index_of_referenced_geometry(n, gcx.geometries, &name) {
        Ok(i) => i,
        Err(p) => {
            p.handled_by("ignoring node", gcx.problems);
            return Err(());
        }
    };
    let offsets = parse_reference_offsets(&n, &name, gcx.problems);

    let geometry = Geometry {
        name,
        t: Type::Reference {
            offsets,
            reference: ref_ind,
        },
    };

    Ok(geometry)
}

// TODO fix warning later, it is only a memory usage problem, due to an enum
// variant in `ProblemType` with many fields
#[allow(clippy::result_large_err)]
fn get_index_of_referenced_geometry(
    n: Node,
    geometries: &mut Geometries,
    name: &Name,
) -> Result<NodeIndex, Problem> {
    let ref_string = n.parse_required_attribute::<Name>("Geometry")?;
    let ref_ind = geometries
        .get_index(&ref_string)
        .ok_or_else(|| ProblemType::UnknownGeometry(ref_string.clone()).at(&n))?;
    if geometries.is_top_level(ref_ind) {
        Ok(ref_ind)
    } else {
        Err(ProblemType::NonTopLevelGeometryReferenced {
            target: ref_string,
            geometry_reference: name.to_owned(),
        }
        .at(&n))
    }
}

fn parse_reference_offsets(&n: &Node, n_name: &Name, problems: &mut Problems) -> Offsets {
    let mut nodes = n
        .children()
        .filter(|n| n.tag_name().name() == "Break")
        .rev(); // start at last element, which provides the Overwrite offset if present

    let mut offsets = Offsets::default();

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
                    geometry_reference: n_name.to_owned(),
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
        use crate::types::dmx_break::Break;

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
                    dmx_break: 1.try_into().unwrap(),
                    offset: 4
                })
            );
            assert_eq!(offsets.normal.len(), 2);
            assert_eq!(offsets.normal[&1.try_into().unwrap()], 1);
            assert_eq!(offsets.normal[&2.try_into().unwrap()], 2);
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
            assert_eq!(offsets.normal[&1.try_into().unwrap()], 6);
            assert_eq!(offsets.normal[&2.try_into().unwrap()], 5);
            assert_eq!(offsets.normal[&3.try_into().unwrap()], 4);
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
                    dmx_break: 3.try_into().unwrap(),
                    offset: 4
                })
            );
            assert_eq!(offsets.normal.len(), 2);
            assert_eq!(offsets.normal[&1.try_into().unwrap()], 6);
            assert_eq!(offsets.normal[&3.try_into().unwrap()], 4);
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
            assert_eq!(offsets.normal[&1.try_into().unwrap()], 6);
            assert_eq!(offsets.normal[&2.try_into().unwrap()], 5);
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
                    duplicate_break,
                    ..
                }
            if duplicate_break == &Break::try_from(2).unwrap()));
            assert_eq!(offsets.normal[&2.try_into().unwrap()], 2); // higher element takes precedence
        }

        #[test]
        fn empty_reference_offsets() {
            let xml = r#"<GeometryReference />"#;
            let (problems, offsets) = run_parse_reference_offsets(xml);
            assert_eq!(problems.len(), 0);
            assert_eq!(offsets, Offsets::default());
        }

        fn run_parse_reference_offsets(xml: &str) -> (Problems, Offsets) {
            let doc = roxmltree::Document::parse(xml).unwrap();
            let n = doc.root_element();
            let mut problems: Problems = vec![];
            let offsets = parse_reference_offsets(
                &n,
                &"arbitrary name for testing".try_into().unwrap(),
                &mut problems,
            );
            (problems, offsets)
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
                <Break DMXBreak="2" DMXOffset="4"/>
                <Break DMXBreak="1" DMXOffset="2"/>
            </GeometryReference>
        </Geometry>
    </Geometries>
</FixtureType>
        "#;

        let (problems, geometries) = run_parse_geometries(ft_str);

        assert!(problems.is_empty());
        assert_eq!(geometries.graph().node_count(), 4);

        let element_2 = &geometries.graph()[geometries.names()[&"Element 2".try_into().unwrap()]];
        if let Geometry {
            name,
            t: Type::Reference { reference, offsets },
        } = element_2
        {
            assert_eq!(name, "Element 2");
            assert_eq!(
                offsets.overwrite,
                Some(Offset {
                    dmx_break: 1.try_into().unwrap(),
                    offset: 2
                })
            );
            assert_eq!(offsets.normal[&1.try_into().unwrap()], 3);
            assert_eq!(offsets.normal[&2.try_into().unwrap()], 4);
            assert_eq!(
                geometries.graph()[reference.to_owned()].name,
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

        let b = geometries.get_index(&"Beam 1".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(b) && geometries.count_children(b) == 0);

        let g2 = geometries
            .get_index(&"Geometry 2".try_into().unwrap())
            .unwrap();
        assert!(geometries.is_top_level(g2) && geometries.count_children(g2) == 0);

        let g3 = geometries
            .get_index(&"Geometry 3".try_into().unwrap())
            .unwrap();
        assert!(geometries.is_top_level(g3));

        let g3_children = geometries.children_geometries(g3).collect::<Vec<_>>();

        assert!(matches!(
            g3_children
                .iter()
                .find(|g| g.name == "Geometry 2 (in Geometry 3)")
                .unwrap(),
            Geometry {
                t: Type::General,
                ..
            }
        ));
        assert!(matches!(
            g3_children.iter().find(|g| g.name == "Geometry 1").unwrap(),
            Geometry {
                t: Type::General,
                ..
            }
        ));
        assert!(matches!(
            g3_children
                .iter()
                .find(|g| g.name == "GeometryReference 3")
                .unwrap(),
            Geometry {
                t: Type::Reference { .. },
                ..
            }
        ));

        assert_eq!(3, g3_children.len());
    }

    #[test]
    fn geometry_duplicate_names() {
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
                    <Geometry Name="Element 1"/> <!-- 5) rename to "Element 1 (in Top 2 (duplicate 1)) (duplicate 1)" -->
                    <Geometry Name="Element 1 (in Top 2 (duplicate 1))"/>
                    <Geometry Name="Top 2"/> <!-- 6) rename to "Top 2 (in Top 2 (duplicate 1))" -->
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

        let t1 = geometries.get_index(&"Top 1".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(t1));
        let t1_children = geometries.children_geometries(t1).collect::<Vec<_>>();
        t1_children.iter().find(|g| g.name == "Element 1").unwrap();
        t1_children
            .iter()
            .find(|g| g.name == "Element 1 (duplicate 1)")
            .unwrap();
        t1_children
            .iter()
            .find(|g| g.name == "Top 2 (in Top 1)")
            .unwrap();
        assert_eq!(t1_children.len(), 3);

        let t2 = geometries.get_index(&"Top 2".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(t2));
        assert_eq!(geometries.count_children(t2), 0);

        let t2d = geometries
            .get_index(&"Top 2 (duplicate 1)".try_into().unwrap())
            .unwrap();
        assert!(geometries.is_top_level(t2d));
        let t2d_children = geometries.children_geometries(t2d).collect::<Vec<_>>();
        t2d_children
            .iter()
            .find(|g| g.name == "Element 1 (in Top 2 (duplicate 1)) (duplicate 1)")
            .unwrap();
        t2d_children
            .iter()
            .find(|g| g.name == "Element 1 (in Top 2 (duplicate 1))")
            .unwrap();
        t2d_children
            .iter()
            .find(|g| g.name == "Top 2 (in Top 2 (duplicate 1))")
            .unwrap();
        assert_eq!(t2d_children.len(), 3);

        let t3 = geometries.get_index(&"Top 3".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(t3));
        let t3_children = geometries.children_geometries(t3).collect::<Vec<_>>();
        let reference = t3_children
            .iter()
            .find(|g| g.name == "Element 1 (in Top 3)")
            .unwrap();
        assert_eq!(t3_children.len(), 1);
        assert!(
            matches!(reference, Geometry { t: Type::Reference{ reference, ..}, .. } if *reference == t2)
        );
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

        assert_eq!(geometries.graph().node_count(), 3);
        assert!(geometries
            .names()
            .contains_key(&"Element 1".try_into().unwrap())
            .not());
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
