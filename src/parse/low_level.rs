use std::{
    io::{Read, Seek},
    num::NonZeroU8,
};

use roxmltree::Node;
use uuid::Uuid;

use crate::{
    low_level::{BasicGeometry, Break, GeometryType, LowLevelGdtf},
    parse_xml::{AssignOrHandle, GetXmlAttribute, GetXmlNode},
    yes_no::YesNoEnum,
    GdtfParseError, HandleProblem, Name, Problem, Problems, ProblemsMut,
};

#[derive(Debug, Default)]
pub struct ParsedGdtf {
    pub gdtf: LowLevelGdtf,
    pub problems: Problems,
}

// TODO impl for ParsedGdtf? Like ParsedGdtf::parse? Or does that intersect with an std trait?
pub fn parse_low_level_gdtf<T: Read + Seek>(reader: T) -> Result<ParsedGdtf, GdtfParseError> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut description_file = zip
        .by_name("description.xml")
        .map_err(GdtfParseError::DescriptionXmlMissing)?;
    let size: usize = description_file.size().try_into().unwrap_or(0);
    let mut description = String::with_capacity(size);
    description_file
        .read_to_string(&mut description)
        .map_err(GdtfParseError::InvalidDescriptionXml)?;
    let low_level_parsed = parse_description(&description)?;
    Ok(low_level_parsed)
}

pub fn parse_description(description: &str) -> Result<ParsedGdtf, super::GdtfParseError> {
    let doc = roxmltree::Document::parse(description)?;
    let gdtf = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(super::GdtfParseError::NoRootNode)?;

    let mut parsed = ParsedGdtf::default();
    parsed.parse_gdtf_root(gdtf);

    Ok(parsed)
}

impl ParsedGdtf {
    fn parse_gdtf_root(&mut self, gdtf: Node) {
        gdtf.parse_required_attribute("DataVersion")
            .assign_or_handle(&mut self.gdtf.data_version, &mut self.problems);

        self.parse_fixture_type(gdtf);
    }

    fn parse_fixture_type(&mut self, gdtf: Node) {
        let fixture_type = match gdtf.find_required_child("FixtureType") {
            Ok(g) => g,
            Err(p) => {
                p.handled_by("returning empty fixture type", self);
                return;
            }
        };

        fixture_type
            .parse_required_attribute("Name")
            .assign_or_handle(&mut self.gdtf.fixture_type.name, &mut self.problems);
        fixture_type
            .parse_required_attribute("ShortName")
            .assign_or_handle(&mut self.gdtf.fixture_type.short_name, &mut self.problems);
        fixture_type
            .parse_required_attribute("LongName")
            .assign_or_handle(&mut self.gdtf.fixture_type.long_name, &mut self.problems);
        fixture_type
            .parse_required_attribute("Manufacturer")
            .assign_or_handle(&mut self.gdtf.fixture_type.manufacturer, &mut self.problems);
        fixture_type
            .parse_required_attribute("Description")
            .assign_or_handle(&mut self.gdtf.fixture_type.description, &mut self.problems);
        fixture_type
            .parse_required_attribute("FixtureTypeID")
            .assign_or_handle(&mut self.gdtf.fixture_type.id, &mut self.problems);

        self.parse_ref_ft(fixture_type);
        self.parse_can_have_children(fixture_type);

        self.parse_geometries(fixture_type);

        // fixture_type
        //     .find_required_child("Geometries")
        //     .ok_or_handled_by("leaving geometries empty", self)
        //     .map(|geometries| self.parse_geometries(geometries));

        // TODO parse Geometries, probably with a good amount of rewriting to adapt to the recursive nature
        // in LowLevelGdtf
        // but should be simpler than in v1

        // GeometriesParser::new(&mut self.gdtf.geometries, &mut self.problems)
        //     .parse_from(&fixture_type);

        // self.parse_dmx_modes(fixture_type);
    }

    /// Parse RefFT attribute
    ///
    /// Even though the DIN requires this attribute, both a missing RefFT
    /// attribute or an empty string (used by GDTF Builder) are parsed to `None`
    /// without raising a Problem. Only invalid UUIDs cause a Problem. This
    /// behavior is useful since both semantically and in practice this
    /// attribute is often absent.
    fn parse_ref_ft(&mut self, fixture_type: Node) {
        self.gdtf.fixture_type.ref_ft = match fixture_type
            .map_parse_attribute::<Uuid, _>("RefFT", |opt| opt.filter(|s| !s.is_empty()))
        {
            Some(Ok(v)) => Some(v),
            Some(Err(p)) => {
                p.handled_by("setting ref_ft to None", self);
                None
            }
            None => None,
        };
    }

    fn parse_can_have_children(&mut self, fixture_type: Node) {
        if let Some(result) = fixture_type.parse_attribute::<YesNoEnum>("CanHaveChildren") {
            result.assign_or_handle(
                &mut self.gdtf.fixture_type.can_have_children,
                &mut self.problems,
            )
        }
    }

    // return Option(()) only to enable early return with `?``
    fn parse_geometries(&mut self, fixture_type: Node<'_, '_>) -> Option<()> {
        let geometries = fixture_type
            .find_required_child("Geometries")
            .ok_or_handled_by("leaving geometries empty", self)?;
        let children = parse_geometry_children(&mut self.problems, geometries);
        self.gdtf.fixture_type.geometries.children.extend(children);

        Some(())
    }
}

impl ProblemsMut for ParsedGdtf {
    fn problems_mut(&mut self) -> &mut Problems {
        &mut self.problems
    }
}

pub(crate) fn parse_geometry_children<'a>(
    p: &'a mut impl ProblemsMut,
    geometry: Node<'a, 'a>,
) -> impl Iterator<Item = GeometryType> + 'a {
    geometry
        .children()
        .filter(|n| n.is_element())
        .enumerate()
        .filter_map(move |(i, n)| {
            let name = n.name(i, p);
            let model = n
                .parse_attribute::<Name>("Model")
                .and_then(|r| r.ok_or_handled_by("using None", p));

            match n.tag_name().name() {
                "Geometry" | "Axis" | "FilterBeam" | "FilterColor" | "FilterGobo"
                | "FilterShaper" | "Beam" | "MediaServerLayer" | "MediaServerCamera"
                | "MediaServerMaster" | "Display" | "Laser" | "WiringObject" | "Inventory"
                | "Structure" | "Support" | "Magnet" => {
                    let children = parse_geometry_children(p, n).collect();
                    Some(GeometryType::Geometry {
                        basic: BasicGeometry { name, model },
                        children,
                    })
                }
                "GeometryReference" => {
                    let breaks = parse_reference_breaks(p, n);
                    let geometry = n
                        .parse_required_attribute("Geometry")
                        .ok_or_handled_by("not parsing node", p)?;
                    Some(GeometryType::GeometryReference {
                        basic: BasicGeometry { name, model },
                        geometry,
                        breaks,
                    })
                }
                other_tag => {
                    Problem::UnexpectedXmlNode(other_tag.into())
                        .at(&n)
                        .handled_by("ignoring node", p);
                    None
                }
            }
        })
}

fn parse_reference_breaks<'a>(
    p: &'a mut impl ProblemsMut,
    geometry_reference: Node<'a, 'a>,
) -> Vec<Break> {
    geometry_reference
        .children()
        .filter(|n| n.is_element())
        .map(|n| {
            let dmx_offset = n
                .parse_attribute("DMXOffset")
                .transpose()
                .ok_or_handled_by("using default 1", p)
                .flatten()
                .unwrap_or_default();
            let dmx_break = n
                .parse_attribute("DMXBreak")
                .and_then(|r| r.ok_or_handled_by("using default 1", p))
                .unwrap_or(NonZeroU8::MIN);
            Break {
                dmx_offset,
                dmx_break,
            }
        })
        .collect()
}
