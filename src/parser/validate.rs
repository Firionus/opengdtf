use crate::{gdtf::gdtf::Gdtf, parse::ParsedGdtf, Problems};

#[derive(Debug)]
pub struct ValidatedGdtf {
    pub gdtf: Gdtf,
    pub problems: Problems,
}

pub fn validate(parsed: ParsedGdtf) -> ValidatedGdtf {
    ValidatedGdtf {
        gdtf: Gdtf {
            data_version: parsed.gdtf.data_version,
            name: parsed.gdtf.fixture_type.name,
            short_name: parsed.gdtf.fixture_type.short_name,
            long_name: parsed.gdtf.fixture_type.long_name,
            manufacturer: parsed.gdtf.fixture_type.manufacturer,
            description: parsed.gdtf.fixture_type.description,
            fixture_type_id: parsed.gdtf.fixture_type.id,
            ref_ft: parsed.gdtf.fixture_type.ref_ft,
            can_have_children: bool::from(parsed.gdtf.fixture_type.can_have_children),
        },
        problems: parsed.problems,
    }
}
