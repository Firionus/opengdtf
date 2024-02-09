use std::io::{Read, Seek};

use crate::{
    gdtf::gdtf::Gdtf,
    parse::{parse_description, ParsedGdtf},
    Error, Problems,
};

#[derive(Debug)]
pub struct ValidatedGdtf {
    pub gdtf: Gdtf,
    pub problems: Problems,
}

pub fn validate(parsed: ParsedGdtf) -> ValidatedGdtf {
    ValidatedGdtf {
        gdtf: Gdtf {
            data_version: parsed.gdtf.data_version,
        },
        problems: parsed.problems,
    }
}
