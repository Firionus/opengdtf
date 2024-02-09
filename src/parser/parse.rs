use std::f32::consts::E;

use quick_xml::{events::Event, Reader};

use crate::{
    low_level_gdtf::low_level_gdtf::LowLevelGdtf,
    parser::problems::{pos, Problem},
};

use super::problems::Problems;

#[derive(Debug)]
pub struct ParsedGdtf {
    pub gdtf: LowLevelGdtf,
    pub problems: Problems,
}

pub(crate) fn parse_description(input: &str) -> ParsedGdtf {
    let mut p = Problems::default();

    let mut r = Reader::from_str(input);

    loop {
        match r.read_event() {
            Ok(Event::Start(t)) => {
                if t.name().0 == b"GDTF" {
                    for Ok(attr) in t.attributes() {
                        todo!()
                        // TODO right here I'm questioning my sanity
                        // the API of quick-xml isn't enjoyable
                        // The push parser from roxmltree would be nicer
                        // or just use roxmltree anyway, it would be easier I think...
                        // We can continue using quick-xml for serialization
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => todo!(),
            Err(e) => {
                Problem::InvalidXml(e, pos(r, &mut p)).handle("aborting parsing", &mut p);
                break;
            }
        }
    }

    todo!()
}
