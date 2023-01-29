use std::collections::{hash_map::Entry::Vacant, HashMap, HashSet, VecDeque};

use petgraph::graph::NodeIndex;
use roxmltree::Node;

use super::{
    parse_xml::{GetXmlAttribute, GetXmlNode},
    problems::HandleProblem,
};

use crate::{
    geometries::Geometries,
    geometry::{Geometry, Offset, Offsets, Type},
    types::{
        dmx_break::Break,
        name::{IntoValidName, Name},
    },
    Problem, ProblemAt, Problems,
};

pub(crate) struct GeometriesParser<'a> {
    geometries: &'a mut Geometries,
    problems: &'a mut Problems,
    /// Queue to hold geometries with duplicate names for later deduplication
    duplicates: VecDeque<Duplicate<'a>>,
    rename_lookup: GeometryLookup,
    renamed_top_level_geometries: HashSet<NodeIndex>,
}

impl<'a> GeometriesParser<'a> {
    pub(crate) fn new(geometries: &'a mut Geometries, problems: &'a mut Problems) -> Self {
        GeometriesParser {
            geometries,
            problems,
            duplicates: Default::default(),
            rename_lookup: Default::default(),
            renamed_top_level_geometries: Default::default(),
        }
    }

    /// Parse the geometries from the fixture type node into geometries.
    ///
    /// Returns a GeometryLookup that later should be used to look up geometries
    /// by name, considering changed names for deduplication.
    pub(crate) fn parse_from(mut self, fixture_type: &'a Node<'a, 'a>) -> GeometryLookup {
        let geometries = match fixture_type.find_required_child("Geometries") {
            Ok(geometries) => geometries,
            Err(p) => {
                p.handled_by("leaving geometries empty", self.problems);
                return Default::default();
            }
        };

        // Top-level geometries must be parsed first so geometry references can link to them
        let top_level_geometries = self.parse_top_level_geometries(geometries);

        for (graph_ind, n) in top_level_geometries.iter() {
            self.add_children(*n, *graph_ind, *graph_ind);
        }

        // handle duplicates after all others, to ensure deduplicated names don't conflict with defined names
        self.parse_duplicates();
        self.rename_lookup
    }

    fn parse_top_level_geometries(
        &mut self,
        geometries: Node<'a, 'a>,
    ) -> Vec<(NodeIndex, Node<'a, 'a>)> {
        let top_level_geometries = geometries.children().filter(|n| n.is_element());
        let mut parsed: Vec<(NodeIndex, Node)> = Default::default();
        for (i, n) in top_level_geometries.enumerate() {
            if let Some((graph_ind, continue_parsing)) = self.parse_element(i, n, None, None) {
                if let Geometry {
                    name,
                    t: Type::Reference { .. },
                } = &self.geometries.graph()[graph_ind]
                {
                    Problem::UnexpectedTopLevelGeometryReference(name.to_owned()).at(&n).handled_by("keeping GeometryReference, \
                    but it is useless because a top-level GeometryReference can only be used for a DMX mode \
                    that is offset from another one, which is useless because one can just change the start address on \
                    the lighting console", self.problems);
                }
                if let ContinueParsing::Children = continue_parsing {
                    parsed.push((graph_ind, n));
                }
            };
        }
        parsed
    }

    /// Recursively adds all children geometries of a parent to geometries
    fn add_children(
        &mut self,
        parent_xml: Node<'a, 'a>,
        parent_graph_ind: NodeIndex,
        top_level_graph_ind: NodeIndex,
    ) {
        let children = parent_xml.children().filter(|n| n.is_element());

        for (i, n) in children.enumerate() {
            if let Some((graph_ind, ContinueParsing::Children)) =
                self.parse_element(i, n, Some(parent_graph_ind), Some(top_level_graph_ind))
            {
                self.add_children(n, graph_ind, top_level_graph_ind)
            }
        }
    }

    /// Parse the geometry element and add it to geometries. If the result is
    /// None or ContinueParsing::No, the caller should not parse the child
    /// elements.
    // TODO should it return Geometry like parse_named_element
    fn parse_element(
        // TODO is fn name good?
        &mut self,
        node_index_in_xml_parent: usize,
        n: Node<'a, 'a>,
        parent_graph_ind: Option<NodeIndex>,
        top_level_graph_ind: Option<NodeIndex>,
    ) -> Option<(NodeIndex, ContinueParsing)> {
        let name = self.name(
            n,
            node_index_in_xml_parent,
            parent_graph_ind,
            top_level_graph_ind,
        )?;

        self.add_named_geometry(n, name, top_level_graph_ind, parent_graph_ind)
    }

    /// Get geometry name or, if name is already present, add to duplicates and
    /// return None.
    fn name(
        &mut self,
        n: Node<'a, 'a>,
        node_index_in_xml_parent: usize,
        parent_graph_ind: Option<NodeIndex>,
        top_level_graph_ind: Option<NodeIndex>,
    ) -> Option<Name> {
        let name = n.name(node_index_in_xml_parent, self.problems);
        match self.geometries.names().get(&name) {
            None => Some(name),
            Some(duplicate_graph_ind) => {
                self.duplicates.push_back(Duplicate {
                    n,
                    parent_graph_ind,
                    top_level_graph_ind,
                    name,
                    duplicate_graph_ind: *duplicate_graph_ind,
                });
                None
            }
        }
    }

    fn add_named_geometry(
        &mut self,
        n: Node,
        name: Name,
        top_level_graph_ind: Option<NodeIndex>,
        parent_graph_ind: Option<NodeIndex>,
    ) -> Option<(NodeIndex, ContinueParsing)> {
        let (geometry, continue_parsing) = {
            match n.tag_name().name() {
                "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo"
                | "FilterShaper" | "Beam" | "MediaServerLayer" | "MediaServerCamera"
                | "MediaServerMaster" | "Display" | "Laser" | "WiringObject" | "Inventory"
                | "Structure" | "Support" | "Magnet" => Some((
                    Geometry {
                        name,
                        t: Type::General,
                    },
                    ContinueParsing::Children,
                )),
                "GeometryReference" => Some((
                    self.named_geometry_reference(n, name, top_level_graph_ind)?,
                    ContinueParsing::No,
                )),
                tag => {
                    Problem::UnexpectedXmlNode(tag.into())
                        .at(&n)
                        .handled_by("ignoring node", self.problems);
                    None
                }
            }
        }?;
        Some((
            self.add_to_geometries(geometry, parent_graph_ind, n)?,
            continue_parsing,
        ))
    }

    fn add_to_geometries(
        &mut self,
        geometry: Geometry,
        parent_graph_ind: Option<NodeIndex>,
        n: Node,
    ) -> Option<NodeIndex> {
        match parent_graph_ind {
            Some(parent_graph_ind) => self.geometries.add(geometry, parent_graph_ind),
            None => self.geometries.add_top_level(geometry),
        }
        .map_err(|err| {
            Problem::Unexpected(err.to_string())
                .at(&n)
                .handled_by("ignoring node", self.problems)
        })
        .ok()
    }

    fn named_geometry_reference(
        &mut self,
        n: Node,
        name: Name,
        top_level_graph_ind: Option<NodeIndex>,
    ) -> Option<Geometry> {
        let reference = self
            .get_index_of_referenced_geometry(n, &name, top_level_graph_ind)
            .ok_or_handled_by("not parsing node", self.problems)?;
        let offsets = parse_reference_offsets(n, &name, self.problems);

        let geometry = Geometry {
            name,
            t: Type::Reference { offsets, reference },
        };

        Some(geometry)
    }

    fn get_index_of_referenced_geometry(
        &mut self,
        n: Node,
        name: &Name,
        top_level_graph_ind: Option<NodeIndex>,
    ) -> Result<NodeIndex, ProblemAt> {
        let ref_string = n.parse_required_attribute::<Name>("Geometry")?;
        let ref_ind = self
            .geometries
            .get_index(&ref_string)
            .ok_or_else(|| Problem::UnknownGeometry(ref_string.clone()).at(&n))?;
        if !self.geometries.is_top_level(ref_ind) {
            return Err(Problem::NonTopLevelGeometryReferenced {
                target: ref_string,
                geometry_reference: name.to_owned(),
            }
            .at(&n));
        }
        if let Some(top_level_graph_ind) = top_level_graph_ind {
            if ref_ind == top_level_graph_ind {
                return Err(Problem::CircularGeometryReference {
                    target: ref_string,
                    geometry_reference: name.to_owned(),
                }
                .at(&n));
            }
        };
        Ok(ref_ind)
    }

    fn parse_duplicates(&mut self) {
        while let Some(dup) = self.duplicates.pop_front() {
            let name_to_increment = match self.try_renaming_with_top_level_name(&dup) {
                Ok(()) => continue,
                Err(name_to_increment) => name_to_increment,
            };

            self.try_renaming_by_incrementing_counter(dup, name_to_increment);
        }
    }

    fn try_renaming_with_top_level_name(&mut self, dup: &Duplicate<'a>) -> Result<(), Name> {
        if let Some(duplicate_top_level) = dup.top_level_graph_ind {
            let original_top_level = self
                .geometries
                .top_level_geometry_index(dup.duplicate_graph_ind);

            if !self
                .renamed_top_level_geometries
                .contains(&duplicate_top_level)
                && original_top_level != duplicate_top_level
            {
                let suggested_name = {
                    let duplicate_name = &dup.name;
                    let duplicate_top_level_name =
                        &self.geometries.graph()[duplicate_top_level].name;
                    format!("{duplicate_name} (in {duplicate_top_level_name})").into_valid()
                };
                if !self.geometries.names().contains_key(&suggested_name) {
                    self.handle_renamed_geometry(dup, &suggested_name, dup.top_level_graph_ind);
                    if let Some(top_level) =
                        self.geometries.graph().node_weight(duplicate_top_level)
                    {
                        self.rename_lookup.insert(
                            (top_level.name.to_owned(), dup.name.clone()),
                            suggested_name.clone(),
                        );
                    } else {
                        Problem::Unexpected(
                            "invalid geometry index in renaming with top level name".into(),
                        )
                        .at(&dup.n)
                        .handled_by(
                            "not putting geometry into lookup, so it might not be found later",
                            self.problems,
                        )
                    }
                    return Ok(());
                }
                return Err(suggested_name);
            }
        };
        Err(dup.name.clone())
    }

    fn try_renaming_by_incrementing_counter(
        &mut self,
        dup: Duplicate<'a>,
        name_to_increment: Name,
    ) {
        for dedup_ind in 1..10_000 {
            let incremented_name =
                format!("{name_to_increment} (duplicate {dedup_ind})").into_valid();
            if !self.geometries.names().contains_key(&incremented_name) {
                self.handle_renamed_geometry(&dup, &incremented_name, dup.top_level_graph_ind);
                return;
            }
        }
        Problem::DuplicateGeometryName(dup.name.clone())
            .at(&dup.n)
            .handled_by("deduplication failed, ignoring node", self.problems);
    }

    fn handle_renamed_geometry(
        &mut self,
        dup: &Duplicate<'a>,
        suggested_name: &Name,
        top_level_graph_ind: Option<NodeIndex>, // TODO isn't this in dup?
    ) {
        Problem::DuplicateGeometryName(dup.name.clone())
            .at(&dup.n)
            .handled_by(format!("renamed to {}", suggested_name), self.problems);

        if let Some((graph_ind, continue_parsing)) = self.add_named_geometry(
            dup.n,
            suggested_name.clone(),
            top_level_graph_ind,
            dup.parent_graph_ind,
        ) {
            if self.geometries.is_top_level(graph_ind) {
                self.renamed_top_level_geometries.insert(graph_ind);
            }

            if let ContinueParsing::Children = continue_parsing {
                self.add_children(dup.n, graph_ind, top_level_graph_ind.unwrap_or(graph_ind));
            }
        }
    }
}

enum ContinueParsing {
    Children,
    No,
}

struct Duplicate<'a> {
    /// already parsed 'Name' attribute on xml_node, can't parse again due to side effects on get_name
    name: Name,
    n: Node<'a, 'a>,
    /// None if duplicate is top-level
    parent_graph_ind: Option<NodeIndex>,
    top_level_graph_ind: Option<NodeIndex>,
    duplicate_graph_ind: NodeIndex,
}

/// (top level name, duplicate geometry name) => renamed name
#[derive(Default, derive_more::DebugCustom, derive_more::IntoIterator)]
pub(crate) struct GeometryLookup(HashMap<(Name, Name), Name>);

#[allow(dead_code)] // TODO remove once dependent code is written
impl GeometryLookup {
    pub(crate) fn deduplicated_name(&self, top_level_geometry: Name, geometry: Name) -> Name {
        match self.0.get(&(top_level_geometry, geometry.clone())) {
            Some(deduplicated_name) => deduplicated_name.clone(),
            None => geometry,
        }
    }

    fn insert(&mut self, k: (Name, Name), v: Name) -> Option<Name> {
        self.0.insert(k, v)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

fn parse_reference_offsets(n: Node, name: &Name, problems: &mut Problems) -> Offsets {
    let mut offsets = Offsets::default();

    let mut nodes = n
        .children()
        .filter(|n| n.tag_name().name() == "Break")
        .rev(); // start at last element, which provides the Overwrite offset if present

    if let Some(last_break) = nodes.next() {
        offsets.overwrite = parse_break(last_break)
            .ok_or_handled_by("ignoring node and setting overwrite to None", problems);
    };

    for n in nodes {
        if let Ok(Offset { dmx_break, offset }) = parse_break(n) {
            if offsets.normal.contains_key(&dmx_break) {
                Problem::DuplicateDmxBreak {
                    duplicate_break: dmx_break,
                    geometry_reference: name.to_owned(),
                }
                .at(&n)
                .handled_by("overwriting previous value", problems)
            }
            offsets.normal.insert(dmx_break, offset);
        }
    }

    // add overwrite to normal if break not already present
    if let Some(Offset { dmx_break, offset }) = offsets.overwrite {
        if let Vacant(entry) = offsets.normal.entry(dmx_break) {
            entry.insert(offset);
        }
    }

    offsets
}

fn parse_break(n: Node) -> Result<Offset, ProblemAt> {
    Ok(Offset {
        dmx_break: n
            .parse_attribute("DMXBreak")
            .unwrap_or_else(|| Ok(Break::default()))?,
        offset: n.parse_attribute("DMXOffset").unwrap_or(Ok(1))?,
    })
}

// allow unwrap/expect eplicitly, because clippy.toml config doesn't work properly yet
// fixed in https://github.com/rust-lang/rust-clippy/pull/9686
// TODO remove once Clippy 0.1.67 is available
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Not;

    #[test]
    fn test_parse_break_node() {
        let xml = r#"<Break DMXBreak="1" DMXOffset="1" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        assert!(matches!(
            parse_break(doc.root_element()),
            Ok(Offset{dmx_break, offset}) if dmx_break.value() == &1u16 && offset == 1
        ));

        let xml = r#"<Break DMXBreak="-1" DMXOffset="1" />"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        assert!(matches!(
            parse_break(doc.root_element()),
            Err(p @ ProblemAt { .. }) if matches!(p.problem(), Problem::InvalidAttribute{attr, ..} if attr=="DMXBreak")
        ));
    }

    #[cfg(test)]
    mod parse_reference_offsets {
        // TODO nesting testing modules, I don't know... Just make a new module if things need separation?
        use crate::types::dmx_break::Break;

        use super::*;

        #[test]
        fn basic_test() {
            // TODO overlap with smoke test below?
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
        fn handles_overwrite_with_missing_argument() {
            let xml = r#"
    <GeometryReference>
        <Break DMXBreak="1" DMXOffset="6"/>
        <Break DMXBreak="2" DMXOffset="5"/>
        <Break MissingDMXBreak="3" DMXOffset="4"/>
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
            assert_eq!(offsets.normal[&1.try_into().unwrap()], 6);
            assert_eq!(offsets.normal[&2.try_into().unwrap()], 5);
        }

        #[test]
        fn handles_overwrite_with_invalid_argument() {
            let xml = r#"
    <GeometryReference>
        <Break DMXBreak="1" DMXOffset="6"/>
        <Break DMXBreak="2" DMXOffset="5"/>
        <Break DMXBreak="0" DMXOffset="4"/> <!-- breaks must be bigger than 0 -->
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
                problems.pop().unwrap().problem(),
                Problem::DuplicateDmxBreak {
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
                n,
                &"arbitrary name for testing".try_into().unwrap(),
                &mut problems,
            );
            (problems, offsets)
        }
    }

    fn name_from(name: &str) -> Name {
        Name::try_from(name).unwrap()
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

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert!(problems.is_empty());
        assert!(rename_lookup.is_empty());
        assert_eq!(geometries.graph().node_count(), 4);

        let abstract_element = geometries.get_index(&name_from("AbstractElement")).unwrap();
        assert!(geometries.is_top_level(abstract_element));

        let main = geometries.get_index(&name_from("Main")).unwrap();
        assert!(geometries.is_top_level(main));
        assert_eq!(geometries.count_children(main), 2);

        let element_1 = geometries.get_index(&name_from("Element 1")).unwrap();
        assert_eq!(geometries.parent_index(element_1).unwrap(), main);

        let element_2 = geometries.get_index(&name_from("Element 2")).unwrap();
        assert_eq!(geometries.parent_index(element_1).unwrap(), main);

        assert!(matches!(
            &geometries.graph()[element_1],
            Geometry {
                t: Type::Reference { reference, offsets },
                ..
            } if reference == &abstract_element
            && matches!(offsets.overwrite, Some(Offset{dmx_break, offset}) if dmx_break.value() == &1 && offset == 1)
            && offsets.normal[&1.try_into().unwrap()] == 1
            && offsets.normal[&2.try_into().unwrap()] == 1
        ));

        assert!(matches!(
            &geometries.graph()[element_2],
            Geometry {
                t: Type::Reference { reference, offsets },
                ..
            } if reference == &abstract_element
            && matches!(offsets.overwrite, Some(Offset{dmx_break, offset}) if dmx_break.value() == &1 && offset == 2)
            && offsets.normal[&1.try_into().unwrap()] == 3
            && offsets.normal[&2.try_into().unwrap()] == 4
        ));
    }
    #[test]
    fn top_level_geometry_reference_and_circular_reference() {
        let ft_str = r#"
    <FixtureType>
        <Geometries>
            <Geometry Name="AbstractElement" 
                Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <GeometryReference Geometry="AbstractElement" Name="Circular Reference" />
            </Geometry>
            <GeometryReference Geometry="AbstractElement" Name="Top Level Reference" 
                Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Break DMXBreak="1" DMXOffset="5"/>
                <Break DMXBreak="2" DMXOffset="6"/>
                <Break DMXBreak="1" DMXOffset="3"/>
            </GeometryReference>
        </Geometries>
    </FixtureType>
            "#;

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert_eq!(problems.len(), 2);

        assert!(matches!(
            problems[0].problem(),
            Problem::UnexpectedTopLevelGeometryReference(name)
            if name.as_str() == "Top Level Reference"
        ));
        assert!(matches!(
            problems[1].problem(),
            Problem::CircularGeometryReference { target, geometry_reference }
            if target.as_str() == "AbstractElement" && geometry_reference == "Circular Reference"
        ));
        assert!(rename_lookup.is_empty());
        assert_eq!(geometries.graph().node_count(), 2);
    }

    #[test]
    /// Geometries should really have a Name attribute, but according to GDTF
    /// 1.2 (DIN SPEC 15800:2022-02), the default value for "Name" type values
    /// is: "object type with an index in parent".
    /// I think the reasonable choice is to still report these problems, but
    /// report which default name was assigned. If there are name duplicates in
    /// default names, they can be handled and report like any other.
    fn missing_geometry_names() {
        let (geometries, rename_lookup, problems) = parse_geometries(
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
                matches!(p.problem(), Problem::XmlAttributeMissing { attr, .. } if attr == "Name")
            );
        }

        assert!(matches!(
            problems[6].problem(),
            Problem::DuplicateGeometryName(dup) if dup == "Geometry 2"
        ));

        assert_eq!(
            rename_lookup.deduplicated_name(name_from("Geometry 3"), name_from("Geometry 2")),
            "Geometry 2 (in Geometry 3)"
        );
        assert_eq!(rename_lookup.into_iter().count(), 1);

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
                    <Geometry Name="Element 1"/> <!-- 2) rename to "Element 1 (duplicate 2)" -->
                    <Geometry Name="Top 2"/> <!-- 3) rename to "Top 2 (in Top 1)" -->
                </Geometry>
                <Geometry Name="Top 2"/>
                <Geometry Name="Top 2"> <!-- 1) rename to "Top 2 (duplicate 1)" -->
                    <Geometry Name="Element 1"/> <!-- 5) rename to "Element 1 (duplicate 3)" -->
                    <Geometry Name="Element 1 (duplicate 1)"/>
                    <Geometry Name="Top 2"/> <!-- 6) rename to "Top 2 (duplicate 2)" -->
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

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert_eq!(problems.len(), 6);
        for p in problems.iter() {
            assert!(matches!(p.problem(), Problem::DuplicateGeometryName(..)))
        }

        // rename lookup only contains duplicates that are uniquely identifiable
        // in a dmx mode through combination of top level geometry and name
        assert_eq!(
            rename_lookup.deduplicated_name(name_from("Top 1"), name_from("Top 2")),
            "Top 2 (in Top 1)"
        );
        assert_eq!(
            rename_lookup.deduplicated_name(name_from("Top 3"), name_from("Element 1")),
            "Element 1 (in Top 3)"
        );
        assert_eq!(rename_lookup.into_iter().count(), 2);

        let t1 = geometries.get_index(&"Top 1".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(t1));
        let t1_children = geometries.children_geometries(t1).collect::<Vec<_>>();
        t1_children.iter().find(|g| g.name == "Element 1").unwrap();
        t1_children
            .iter()
            .find(|g| g.name == "Element 1 (duplicate 2)")
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
            .find(|g| g.name == "Element 1 (duplicate 1)")
            .unwrap();
        t2d_children
            .iter()
            .find(|g| g.name == "Element 1 (duplicate 3)")
            .unwrap();
        t2d_children
            .iter()
            .find(|g| g.name == "Top 2 (duplicate 2)")
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

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert!(rename_lookup.is_empty());
        assert_eq!(problems.len(), 1);
        assert!(matches!(
            problems[0].problem(),
            Problem::NonTopLevelGeometryReferenced { .. },
        ));

        assert_eq!(geometries.graph().node_count(), 3);
        assert!(geometries
            .names()
            .contains_key(&"Element 1".try_into().unwrap())
            .not());
    }

    fn parse_geometries(ft_str: &str) -> (Geometries, GeometryLookup, Problems) {
        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Problems = vec![];
        let mut geometries = Geometries::default();
        let rename_lookup = GeometriesParser::new(&mut geometries, &mut problems).parse_from(&ft);
        (geometries, rename_lookup, problems)
    }
}
