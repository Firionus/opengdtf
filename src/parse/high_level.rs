use std::io::{Read, Seek};

use getset::Getters;

use crate::{Gdtf, GdtfParseError, Problems};

use super::low_level::ParsedGdtf;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct ValidatedGdtf {
    gdtf: Gdtf,
    problems: Problems,
}

impl ValidatedGdtf {
    pub fn from_reader<T: Read + Seek>(reader: T) -> Result<Self, GdtfParseError> {
        let low_level_parsed = ParsedGdtf::from_reader(reader)?;
        Ok(validate(low_level_parsed))
    }
}

pub fn validate(parsed: ParsedGdtf) -> ValidatedGdtf {
    let mut gdtf = Gdtf::default();
    gdtf.data_version = parsed.gdtf.data_version;
    gdtf.name = parsed.gdtf.fixture_type.name;
    gdtf.short_name = parsed.gdtf.fixture_type.short_name;
    gdtf.long_name = parsed.gdtf.fixture_type.long_name;
    gdtf.manufacturer = parsed.gdtf.fixture_type.manufacturer;
    gdtf.description = parsed.gdtf.fixture_type.description;
    gdtf.fixture_type_id = parsed.gdtf.fixture_type.id;
    gdtf.ref_ft = parsed.gdtf.fixture_type.ref_ft;
    gdtf.can_have_children = bool::from(parsed.gdtf.fixture_type.can_have_children);
    // TODO geometries
    ValidatedGdtf {
        gdtf,
        problems: parsed.problems,
    }
}
