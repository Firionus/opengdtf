#![allow(clippy::result_large_err)]
// TODO fix warning later, it is only a memory usage problem, due to an enum
// variant in `ProblemType` with many fields
mod errors;
mod geometries;
mod parse_xml;
mod problems;
mod types;

use std::io::{Read, Seek};

use roxmltree::Node;
use uuid::Uuid;

use crate::Gdtf;

pub use self::{
    errors::Error,
    problems::{HandledProblem, Problem, ProblemAt, Problems},
};

use self::{
    geometries::GeometriesParser,
    parse_xml::{get_xml_attribute::GetXmlAttribute, AssignOrHandle, GetXmlNode},
    types::yes_no::YesNoEnum,
};

#[derive(Debug, Default)]
pub struct ParsedGdtf {
    pub gdtf: Gdtf,
    pub problems: Problems,
}

pub fn parse<T: Read + Seek>(reader: T) -> Result<ParsedGdtf, Error> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut description_file = zip
        .by_name("description.xml")
        .map_err(Error::DescriptionXmlMissing)?;

    let size: usize = description_file.size().try_into().unwrap_or(0);
    let mut description = String::with_capacity(size);

    description_file
        .read_to_string(&mut description)
        .map_err(Error::InvalidDescriptionXml)?;

    parse_description(description)
}

fn parse_description(description: String) -> Result<ParsedGdtf, Error> {
    let doc = roxmltree::Document::parse(&description)?;
    let gdtf = doc
        .descendants()
        .find(|n| n.has_tag_name("GDTF"))
        .ok_or(Error::NoRootNode)?;

    let mut parsed = ParsedGdtf::default();
    parsed.parse(gdtf);

    Ok(parsed)
}

impl ParsedGdtf {
    fn parse(&mut self, gdtf: Node) {
        gdtf.parse_required_attribute("DataVersion")
            .assign_or_handle(&mut self.gdtf.data_version, &mut self.problems);

        self.parse_fixture_type(gdtf);
    }

    fn parse_fixture_type(&mut self, gdtf: Node) {
        let fixture_type = match gdtf.find_required_child("FixtureType") {
            Ok(g) => g,
            Err(p) => {
                p.handled_by("returning empty fixture type", &mut self.problems);
                return;
            }
        };

        fixture_type
            .parse_required_attribute("FixtureTypeID")
            .assign_or_handle(&mut self.gdtf.fixture_type_id, &mut self.problems);
        fixture_type
            .parse_required_attribute("Name")
            .assign_or_handle(&mut self.gdtf.name, &mut self.problems);
        fixture_type
            .parse_required_attribute("ShortName")
            .assign_or_handle(&mut self.gdtf.short_name, &mut self.problems);
        fixture_type
            .parse_required_attribute("LongName")
            .assign_or_handle(&mut self.gdtf.long_name, &mut self.problems);
        fixture_type
            .parse_required_attribute("Description")
            .assign_or_handle(&mut self.gdtf.description, &mut self.problems);
        fixture_type
            .parse_required_attribute("Manufacturer")
            .assign_or_handle(&mut self.gdtf.manufacturer, &mut self.problems);

        self.parse_ref_ft(fixture_type);
        self.parse_can_have_children(fixture_type);

        GeometriesParser::new(&mut self.gdtf.geometries, &mut self.problems)
            .parse_from(&fixture_type);
    }

    /// Parse RefFT attribute
    ///
    /// Even though the DIN requires this attribute, both a missing RefFT
    /// attribute or an empty string (used by GDTF Builder) are parsed to `None`
    /// without raising a Problem. Only invalid UUIDs cause a Problem. This
    /// behavior is useful since both semantically and in practice this
    /// attribute is often absent.
    fn parse_ref_ft(&mut self, fixture_type: Node) {
        self.gdtf.ref_ft = match fixture_type
            .map_parse_attribute::<Uuid, _>("RefFT", |opt| opt.filter(|s| !s.is_empty()))
        {
            Some(Ok(v)) => Some(v),
            Some(Err(p)) => {
                p.handled_by("setting ref_ft to None", &mut self.problems);
                None
            }
            None => None,
        };
    }

    fn parse_can_have_children(&mut self, fixture_type: Node) {
        if let Some(result) = fixture_type.parse_attribute::<YesNoEnum>("CanHaveChildren") {
            result
                .map(bool::from)
                .assign_or_handle(&mut self.gdtf.can_have_children, &mut self.problems)
        }
    }
}

// allow unwrap/expect eplicitly, because clippy.toml config doesn't work properly yet
// fixed in https://github.com/rust-lang/rust-clippy/pull/9686
// TODO remove once Clippy 0.1.67 is available
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use roxmltree::Document;

    use super::*;

    #[test]
    fn xml_error() {
        let invalid_xml = "<this></that>".to_string();
        let res = parse_description(invalid_xml);
        assert!(matches!(res, Err(Error::InvalidXml(..))));
    }

    #[test]
    fn no_root_node_error() {
        let invalid_xml = "<this></this>".to_string();
        let res = parse_description(invalid_xml);
        assert!(matches!(res, Err(Error::NoRootNode)));
    }

    #[test]
    fn test_parsing_ref_ft() {
        assert_ref_ft_after_parsing(r#"<FixtureType />"#, None, 0);
        assert_ref_ft_after_parsing(r#"<FixtureType RefFT="" />"#, None, 0);
        assert_ref_ft_after_parsing(
            r#"<FixtureType RefFT="00000000-0000-0000-0000-000000000000" />"#,
            Some(Uuid::nil()),
            0,
        );
        assert_ref_ft_after_parsing(r#"<FixtureType RefFT="this is not a UUID" />"#, None, 1);
    }

    fn assert_ref_ft_after_parsing(input: &str, expected: Option<Uuid>, expected_problems: usize) {
        let doc = Document::parse(input).unwrap();
        let n = doc.root().first_element_child().unwrap();
        assert!(n.has_tag_name("FixtureType"));
        let mut parsed = ParsedGdtf::default();

        // ensure we don't just test against the default value of None
        parsed.gdtf.ref_ft = Some(Uuid::parse_str("eff34b75-c498-4265-896d-6d390fc39143").unwrap());

        parsed.parse_ref_ft(n);
        assert_eq!(parsed.gdtf.ref_ft, expected);
        assert_eq!(parsed.problems.len(), expected_problems);
    }

    #[test]
    fn test_parsing_can_have_children() {
        assert_can_have_children_after_parsing(r#"<FixtureType />"#, true, 0);
        assert_can_have_children_after_parsing(r#"<FixtureType CanHaveChildren="Yes" />"#, true, 0);
        assert_can_have_children_after_parsing(r#"<FixtureType CanHaveChildren="No" />"#, false, 0);
        assert_can_have_children_after_parsing(
            r#"<FixtureType CanHaveChildren="Not Yes or No but some other String" />"#,
            true,
            1,
        );
    }

    fn assert_can_have_children_after_parsing(
        input: &str,
        expected: bool,
        expected_problems: usize,
    ) {
        let doc = Document::parse(input).unwrap();
        let n = doc.root().first_element_child().unwrap();
        assert!(n.has_tag_name("FixtureType"));
        let mut parsed = ParsedGdtf::default();
        parsed.parse_can_have_children(n);
        assert_eq!(parsed.gdtf.can_have_children, expected);
        assert_eq!(parsed.problems.len(), expected_problems);
    }
}
