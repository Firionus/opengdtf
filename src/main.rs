pub mod gdtf;
pub mod low_level_gdtf;
pub mod parser;

use quick_xml::de;
use serde::{Deserialize, Serialize};

use crate::parser::parse::parse_description;

fn main() {
    println!("Start Parsing");

    let input = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
    <GDTF DataVersion="1.2" arb="what?">
    <FixtureType CanHaveChildren="Yes" Description="ARRI Orbiter  Illumination|Reshaped" FixtureTypeID="70C79926-9513-430F-A71C-52662FA1EC70" LongName="ARRI Orbiter" Manufacturer="ARRI" Name="Orbiter" RefFT="" ShortName="Orbiter" Thumbnail="thumbnail" ThumbnailOffsetX="913" ThumbnailOffsetY="125248448">
    </FixtureType>
    </GDTF>"#;
    let gdtf = parse_description(input);
    println!("{gdtf:#?}");
    // println!(
    //     "Data Version with Display looks like this: {}",
    //     gdtf.data_version
    // );
}
