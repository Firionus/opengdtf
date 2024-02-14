use std::io::{Read, Seek};

use crate::{Gdtf, GdtfParseError, Problems};

use super::low_level::{parse_low_level_gdtf, ParsedGdtf};

// TODO turn into impl on ValidatedGdtf?
pub fn parse_gdtf<T: Read + Seek>(reader: T) -> Result<ValidatedGdtf, GdtfParseError> {
    let low_level_parsed = parse_low_level_gdtf(reader)?;
    Ok(validate(low_level_parsed))
}

#[derive(Debug)]
pub struct ValidatedGdtf {
    pub gdtf: Gdtf,
    pub problems: Problems,
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
