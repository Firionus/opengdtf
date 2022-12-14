mod errors;
mod geometries;
mod utils;

use std::io::{Read, Seek};

use strum::EnumString;
use uuid::Uuid;

use crate::{parser::utils::AssignOrHandle, Gdtf};

pub use self::errors::{Error, Problem};

use self::{
    errors::{HandledProblem, ProblemType},
    geometries::parse_geometries,
    utils::GetFromNode,
};

pub type Problems = Vec<HandledProblem>;

#[derive(Debug)]
pub struct Parsed {
    pub gdtf: Gdtf,
    pub problems: Problems,
}

pub fn parse<T: Read + Seek>(reader: T) -> Result<Parsed, Error> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut description_file = zip
        .by_name("description.xml")
        .map_err(Error::DescriptionXmlMissing)?;
    let mut description = String::new();
    description_file
        .read_to_string(&mut description)
        .map_err(Error::InvalidDescriptionXml)?;

    parse_description(&description[..])
}

fn parse_description(description_content: &str) -> Result<Parsed, Error> {
    let doc = roxmltree::Document::parse(description_content)?;

    let mut gdtf = Gdtf::default();

    let mut problems: Problems = vec![];

    let root_node = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(Error::NoRootNode)?;

    root_node
        .parse_required_attribute("DataVersion")
        .assign_or_handle(&mut gdtf.data_version, &mut problems);
    // TODO communicate how we handle version that aren't v1.2 here, if applicable

    let ft = match root_node.children().find(|n| n.has_tag_name("FixtureType")) {
        Some(ft) => ft,
        None => {
            ProblemType::XmlNodeMissing {
                missing: "FixtureType".to_owned(),
                parent: "GDTF".to_owned(),
            }
            .at(&root_node)
            .handled_by("returning empty fixture type", &mut problems);
            return Ok(Parsed { gdtf, problems });
        }
    };

    let geometries = &mut gdtf.geometries;

    parse_geometries(geometries, &ft, &mut problems)?;

    ft.parse_required_attribute("FixtureTypeID")
        .assign_or_handle(&mut gdtf.fixture_type_id, &mut problems);

    // empty string as ref_ft is not considered a problem, just parses to None
    // while the DIN requires this attribute, semantically and in practice it is usually absent and the GDTF builder encodes absence as empty string
    // TODO test this behavior
    gdtf.ref_ft =
        match ft.map_parse_attribute::<Uuid, _>("RefFT", |opt| opt.filter(|s| !s.is_empty())) {
            Some(Ok(v)) => Some(v),
            Some(Err(p)) => {
                p.handled_by("setting ref_ft to None", &mut problems);
                None
            }
            None => None,
        };

    // TODO test this, I wanna see the error output :)
    match ft.parse_attribute::<YesNoEnum>("CanHaveChildren") {
        Some(Ok(v)) => gdtf.can_have_children = v.into(),
        Some(Err(p)) => p.handled_by(
            format!("using default value {}", gdtf.can_have_children),
            &mut problems,
        ),
        None => (),
    }

    ft.parse_required_attribute("Name")
        .assign_or_handle(&mut gdtf.name, &mut problems);

    ft.parse_required_attribute("ShortName")
        .assign_or_handle(&mut gdtf.short_name, &mut problems);

    ft.parse_required_attribute("LongName")
        .assign_or_handle(&mut gdtf.long_name, &mut problems);

    ft.parse_required_attribute("Description")
        .assign_or_handle(&mut gdtf.description, &mut problems);

    ft.parse_required_attribute("Manufacturer")
        .assign_or_handle(&mut gdtf.manufacturer, &mut problems);

    Ok(Parsed { gdtf, problems })
}

#[derive(strum::Display, EnumString)]
enum YesNoEnum {
    #[strum(to_string = "Yes")]
    Yes,
    #[strum(to_string = "No")]
    No,
}

impl From<YesNoEnum> for bool {
    fn from(value: YesNoEnum) -> Self {
        match value {
            YesNoEnum::Yes => true,
            YesNoEnum::No => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_error() {
        let invalid_xml = "<this></that>";
        let res = parse_description(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, Error::InvalidXml(..)));
        let msg: String = format!("{}", e);
        assert!(msg == "invalid XML: expected 'this' tag, not 'that' at 1:7");
    }

    #[test]
    fn no_root_node_error() {
        let invalid_xml = "<this></this>";
        let res = parse_description(invalid_xml);
        let e = res.unwrap_err();
        assert!(matches!(&e, Error::NoRootNode));
    }
}
