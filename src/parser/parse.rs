use std::io::{Read, Seek};

use roxmltree::Node;
use uuid::Uuid;

use crate::{
    low_level_gdtf::low_level_gdtf::LowLevelGdtf, problems::ProblemsMut, validate::validate,
    yes_no::YesNoEnum, Error, ValidatedGdtf,
};

use super::{
    parse_xml::{get_xml_node::GetXmlNode, AssignOrHandle, GetXmlAttribute},
    problems::Problems,
};

#[derive(Debug, Default)]
pub struct ParsedGdtf {
    pub gdtf: LowLevelGdtf,
    pub problems: Problems,
}

pub fn parse_gdtf<T: Read + Seek>(reader: T) -> Result<ValidatedGdtf, Error> {
    let low_level_parsed = parse_low_level_gdtf(reader)?;
    Ok(validate(low_level_parsed))
}

pub fn parse_low_level_gdtf<T: Read + Seek>(reader: T) -> Result<ParsedGdtf, Error> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut description_file = zip
        .by_name("description.xml")
        .map_err(Error::DescriptionXmlMissing)?;
    let size: usize = description_file.size().try_into().unwrap_or(0);
    let mut description = String::with_capacity(size);
    description_file
        .read_to_string(&mut description)
        .map_err(Error::InvalidDescriptionXml)?;
    let low_level_parsed = parse_description(&description)?;
    Ok(low_level_parsed)
}

pub fn parse_description(description: &str) -> Result<ParsedGdtf, super::Error> {
    let doc = roxmltree::Document::parse(description)?;
    let gdtf = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(super::Error::NoRootNode)?;

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
}

impl ProblemsMut for ParsedGdtf {
    fn problems_mut(&mut self) -> &mut Problems {
        &mut self.problems
    }
}
