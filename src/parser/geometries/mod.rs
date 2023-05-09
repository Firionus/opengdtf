use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::graph::NodeIndex;
use roxmltree::Node;

use self::{deduplication::Duplicate, reference::DeferredReference};

use super::parse_xml::{GetXmlAttribute, GetXmlNode};

use crate::{
    geometries::Geometries,
    geometry::{Geometry, Type},
    name::Name,
    Problem, Problems,
};

mod deduplication;
mod reference;

pub(crate) struct GeometriesParser<'a> {
    geometries: &'a mut Geometries,
    problems: &'a mut Problems,
    references: VecDeque<DeferredReference<'a>>,
    duplicates: VecDeque<Duplicate<'a>>,
    renamed_top_level_geometries: HashSet<NodeIndex>,
    rename_lookup: GeometryLookup,
}

/// maps (top level name, duplicate geometry name) => renamed name
#[derive(Default, derive_more::DebugCustom, derive_more::IntoIterator)]
pub(crate) struct GeometryLookup(HashMap<(Name, Name), Name>);

impl<'a> GeometriesParser<'a> {
    pub(crate) fn new(geometries: &'a mut Geometries, problems: &'a mut Problems) -> Self {
        GeometriesParser {
            geometries,
            problems,
            references: Default::default(),
            duplicates: Default::default(),
            renamed_top_level_geometries: Default::default(),
            rename_lookup: Default::default(),
        }
    }

    /// Parse the geometries from the fixture type node into geometries.
    ///
    /// Returns a GeometryLookup that later should be used to look up geometries
    /// by name, considering names changed for deduplication.
    pub(crate) fn parse_from(mut self, fixture_type: &'a Node<'a, 'a>) -> GeometryLookup {
        let geometries = match fixture_type.find_required_child("Geometries") {
            Ok(geometries) => geometries,
            Err(p) => {
                p.handled_by("leaving geometries empty", self.problems);
                return self.rename_lookup;
            }
        };

        let top_level_geometries = geometries.children().filter(|n| n.is_element());
        for (i, n) in top_level_geometries.enumerate() {
            if let Some((graph_ind, continue_parsing)) = self.parse_geometry(i, n, None, None) {
                if let Some(Geometry {
                    name,
                    t: Type::Reference { .. },
                }) = &self.geometries.graph().node_weight(graph_ind)
                {
                    Problem::UnexpectedTopLevelGeometryReference(name.to_owned()).at(&n).handled_by("keeping GeometryReference, \
                    but it is useless because a top-level GeometryReference can only be used for a DMX mode \
                    that is offset from another one, which is useless because one can just change the start address on \
                    the lighting console", self.problems);
                }
                if let ContinueParsing::Children = continue_parsing {
                    self.add_children(n, graph_ind, graph_ind);
                }
            };
        }

        while !self.references.is_empty() | !self.duplicates.is_empty() {
            self.parse_references();
            // handle duplicates after all others, to ensure deduplicated names don't conflict with defined names
            self.parse_duplicates();
        }

        self.rename_lookup
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
                self.parse_geometry(i, n, Some(parent_graph_ind), Some(top_level_graph_ind))
            {
                self.add_children(n, graph_ind, top_level_graph_ind)
            }
        }
    }

    /// Parse the geometry element and add it to geometries. If the result is
    /// None or ContinueParsing::No, the caller should not parse the child
    /// elements.
    fn parse_geometry(
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

        self.add_named_geometry(n, name, parent_graph_ind)
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
                self.duplicates.push_back(Duplicate::new(
                    name,
                    n,
                    parent_graph_ind,
                    top_level_graph_ind,
                    *duplicate_graph_ind,
                ));
                None
            }
        }
    }

    fn add_named_geometry(
        &mut self,
        n: Node<'a, 'a>,
        name: Name,
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
                "GeometryReference" => {
                    Some((self.named_geometry_reference(n, name)?, ContinueParsing::No))
                }
                tag => {
                    Problem::UnexpectedXmlNode(tag.into())
                        .at(&n)
                        .handled_by("ignoring node", self.problems);
                    None
                }
            }
        }?;
        let graph_ind = self.add_to_geometries(geometry, parent_graph_ind, n)?;
        Some((graph_ind, continue_parsing))
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
            Problem::Unexpected(err.into())
                .at(&n)
                .handled_by("ignoring node", self.problems)
        })
        .ok()
    }
}

enum ContinueParsing {
    Children,
    No,
}

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

#[cfg(test)]
mod tests {
    use petgraph::Direction::Incoming;

    use crate::{geometry::Offset, name::IntoValidName};

    use super::*;

    fn parse_geometries(ft_str: &str) -> (Geometries, GeometryLookup, Problems) {
        let doc = roxmltree::Document::parse(ft_str).unwrap();
        let ft = doc.root_element();
        let mut problems: Problems = vec![];
        let mut geometries = Geometries::default();
        let rename_lookup = GeometriesParser::new(&mut geometries, &mut problems).parse_from(&ft);
        (geometries, rename_lookup, problems)
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

        let abstract_element = geometries
            .get_index(&"AbstractElement".into_valid())
            .unwrap();
        assert!(geometries.is_top_level(abstract_element));

        let main = geometries.get_index(&"Main".into_valid()).unwrap();
        assert!(geometries.is_top_level(main));
        assert_eq!(geometries.count_children(main), 2);

        let element_1 = geometries.get_index(&"Element 1".into_valid()).unwrap();
        assert_eq!(geometries.parent_index(element_1).unwrap(), main);

        let element_2 = geometries.get_index(&"Element 2".into_valid()).unwrap();
        assert_eq!(geometries.parent_index(element_1).unwrap(), main);

        assert!(matches!(
            &geometries.graph().node_weight(element_1).unwrap(),
            Geometry {
                t: Type::Reference { offsets },
                ..
            } if matches!(offsets.overwrite, Some(Offset{dmx_break, offset}) if dmx_break.value() == &1 && offset == 1)
            && offsets.normal.get(&1.try_into().unwrap()).unwrap() == &1
            && offsets.normal.get(&2.try_into().unwrap()).unwrap() == &1
        ));
        assert!(matches!(
            geometries
                .templates()
                .neighbors_directed(element_1, Incoming)
                .next(),
            Some(abstract_element) if abstract_element == abstract_element
        ));

        assert!(matches!(
            &geometries.graph().node_weight(element_2).unwrap(),
            Geometry {
                t: Type::Reference { offsets },
                ..
            } if matches!(offsets.overwrite, Some(Offset{dmx_break, offset}) if dmx_break.value() == &1 && offset == 2)
            && offsets.normal.get(&1.try_into().unwrap()).unwrap() == &3
            && offsets.normal.get(&2.try_into().unwrap()).unwrap() == &4
        ));
        assert!(matches!(
            geometries
                .templates()
                .neighbors_directed(element_2, Incoming)
                .next(),
            Some(abstract_element) if abstract_element == abstract_element
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

        let mut problems = problems.iter();
        assert!(matches!(
            problems.next().unwrap().problem(),
            Problem::UnexpectedTopLevelGeometryReference(name)
            if name.as_str() == "Top Level Reference"
        ));
        assert!(matches!(
            problems.next().unwrap().problem(),
            Problem::InvalidGeometryReference(..)
        ));
        assert!(rename_lookup.is_empty());
        assert_eq!(geometries.graph().node_count(), 3);
        assert_eq!(geometries.templates().edge_count(), 1);
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

        let problems = problems.iter();
        for p in problems.clone().take(6) {
            assert!(
                matches!(p.problem(), Problem::XmlAttributeMissing { attr, .. } if attr == "Name")
            );
        }

        assert!(matches!(
            problems.last().unwrap().problem(),
            Problem::DuplicateGeometryName(dup) if dup == "Geometry 2"
        ));

        assert_eq!(
            rename_lookup.deduplicated_name("Geometry 3".into_valid(), "Geometry 2".into_valid()),
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
                    <Geometry Name="Element 1"/> <!-- rename to "Element 1 (duplicate 1)" -->
                    <Geometry Name="Top 2"/>
                </Geometry>
                <Geometry Name="Top 2"/> <!-- rename to "Top 2 (duplicate 1)" -->
                <Geometry Name="Top 2"> <!-- rename to "Top 2 (duplicate 2)" -->
                    <Geometry Name="Element 1"/> <!-- rename to "Element 1 (duplicate 2)", though it might only make limited sense -->
                    <Geometry Name="Element 1 (duplicate 1)"/> <!-- rename to "Element 1 (duplicate 1) (duplicate 1)" -->
                    <Geometry Name="Top 2"/> <!-- rename to "Top 2 (duplicate 3)" -->
                </Geometry>
                <Geometry Name="Top 3">
                    <GeometryReference Geometry="Top 1" Name="Element 1"> <!-- rename to "Element 1 (in Top 3)" -->
                        <Break DMXBreak="1" DMXOffset="1"/>
                    </GeometryReference>
                </Geometry>
            </Geometries>
        </FixtureType>
                "#;

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert_eq!(problems.len(), 7);
        for p in problems.iter() {
            assert!(matches!(p.problem(), Problem::DuplicateGeometryName(..)))
        }

        // rename lookup only contains duplicates that are uniquely identifiable
        // in a dmx mode through combination of top level geometry and name
        assert_eq!(
            rename_lookup.deduplicated_name("Top 3".into_valid(), "Element 1".into_valid()),
            "Element 1 (in Top 3)"
        );
        assert_eq!(rename_lookup.into_iter().count(), 1);

        let t1 = geometries.get_index(&"Top 1".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(t1));
        let t1_children = geometries.children_geometries(t1).collect::<Vec<_>>();
        t1_children.iter().find(|g| g.name == "Element 1").unwrap();
        t1_children
            .iter()
            .find(|g| g.name == "Element 1 (duplicate 1)")
            .unwrap();
        t1_children.iter().find(|g| g.name == "Top 2").unwrap();
        assert_eq!(t1_children.len(), 3);

        let t2d1 = geometries
            .get_index(&"Top 2 (duplicate 1)".try_into().unwrap())
            .unwrap();
        assert!(geometries.is_top_level(t2d1));
        assert_eq!(geometries.count_children(t2d1), 0);

        let t2d2 = geometries
            .get_index(&"Top 2 (duplicate 2)".try_into().unwrap())
            .unwrap();
        assert!(geometries.is_top_level(t2d2));
        let t2d2_children = geometries.children_geometries(t2d2).collect::<Vec<_>>();
        t2d2_children
            .iter()
            .find(|g| g.name == "Element 1 (duplicate 2)")
            .unwrap();
        t2d2_children
            .iter()
            .find(|g| g.name == "Element 1 (duplicate 1) (duplicate 1)")
            .unwrap();
        t2d2_children
            .iter()
            .find(|g| g.name == "Top 2 (duplicate 3)")
            .unwrap();
        assert_eq!(t2d2_children.len(), 3);

        let t3 = geometries.get_index(&"Top 3".try_into().unwrap()).unwrap();
        assert!(geometries.is_top_level(t3));
        let mut t3_children = geometries.graph().neighbors(t3);
        let reference = t3_children.next().unwrap();
        assert_eq!(
            geometries.get_by_index(reference).unwrap().name,
            "Element 1 (in Top 3)"
        );
        assert!(matches!(
            geometries
                .templates()
                .neighbors_directed(reference, Incoming)
                .next(),
            Some(i) if i == t1
        ));
        assert_eq!(t3_children.next(), None);
    }

    #[test]
    fn geometry_reference_to_non_top_level_geometry() {
        let ft_str = r#"
    <FixtureType>
        <Geometries>
            <Geometry Name="Main 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Geometry Name="AbstractElement 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
            </Geometry>
            <Geometry Name="Main 2" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <GeometryReference Geometry="AbstractElement 1" Name="Element 1" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}" />
                <GeometryReference Geometry="AbstractElement 2" Name="Element 2" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}" />
                <GeometryReference Geometry="Element 1" Name="Element 3" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}" />
            </Geometry>
            <Geometry Name="Main 3" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}">
                <Geometry Name="AbstractElement 2" Position="{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}{0.000000,0.000000,1.000000,0.000000}{0,0,0,1}"/>
            </Geometry>
        </Geometries>
    </FixtureType>
            "#;

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert!(rename_lookup.is_empty());
        assert_eq!(problems.len(), 3);
        for p in problems {
            assert!(matches!(p.problem(), Problem::InvalidGeometryReference(..)))
        }

        assert_eq!(geometries.graph().node_count(), 8);
        assert_eq!(geometries.templates().edge_count(), 0);
    }

    #[test]
    fn top_level_geometry_reference_must_give_right_error_type() {
        let ft_str = r#"
    <FixtureType>
        <Geometries>
            <GeometryReference Geometry="Main 1" Name="Element 1" />
            <Geometry Name="Main 1" />
            <GeometryReference Geometry="Main 1" Name="Element 2" />
        </Geometries>
    </FixtureType>
            "#;

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert!(rename_lookup.is_empty());
        assert_eq!(problems.len(), 2);
        for p in problems {
            assert!(matches!(
                p.problem(),
                Problem::UnexpectedTopLevelGeometryReference { .. }
            ))
        }

        assert_eq!(geometries.graph().node_count(), 3);
    }

    #[test]
    fn geometry_reference_chains() {
        let ft_str = r#"
    <FixtureType>
        <Geometries>
            <GeometryReference Geometry="Main 1" Name="Element 1" />
            <Geometry Name="Main 1" />
            <Geometry Name="Main 2">
                <GeometryReference Geometry="Element 1" Name="Element 2" />
            </Geometry>
        </Geometries>
    </FixtureType>
            "#;

        let (geometries, rename_lookup, problems) = parse_geometries(ft_str);

        assert!(rename_lookup.is_empty());
        assert_eq!(problems.len(), 2);
        let mut problems_iter = problems.iter();
        assert!(matches!(
                problems_iter.next().unwrap().problem(), 
                Problem::UnexpectedTopLevelGeometryReference(name) 
                if name == &"Element 1".into_valid()));
        assert!(matches!(
            problems_iter.next().unwrap().problem(),
            Problem::InvalidGeometryReference(..)
        ));

        assert_eq!(geometries.graph().node_count(), 4);
        assert_eq!(geometries.templates().edge_count(), 1); // Main 1 -> Element 1 is kept but useless
    }
}
