use std::{
    f32::consts::E,
    io::{Read, Seek},
};

use quick_xml::{events::Event, Reader};
use roxmltree::Node;

use crate::{
    low_level_gdtf::low_level_gdtf::LowLevelGdtf, parser::problems::Problem, validate::validate,
    Error, ValidatedGdtf,
};

use super::{
    parse_xml::{AssignOrHandle, GetXmlAttribute},
    problems::Problems,
};

#[derive(Debug, Default)]
pub struct ParsedGdtf {
    pub gdtf: LowLevelGdtf,
    pub problems: Problems,
}

pub fn parse<T: Read + Seek>(reader: T) -> Result<ValidatedGdtf, Error> {
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
    Ok(validate(low_level_parsed))
}

pub fn parse_description(description: &str) -> Result<ParsedGdtf, super::Error> {
    let doc = roxmltree::Document::parse(&description)?;
    let gdtf = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(super::Error::NoRootNode)?;

    let mut parsed = ParsedGdtf::default();
    parsed.parse(gdtf);

    Ok(parsed)
}

impl ParsedGdtf {
    fn parse(&mut self, gdtf: Node) {
        gdtf.parse_required_attribute("DataVersion")
            .assign_or_handle(&mut self.gdtf.data_version, &mut self.problems);

        // TODO uncomment
        //self.parse_fixture_type(gdtf);
    }
}
