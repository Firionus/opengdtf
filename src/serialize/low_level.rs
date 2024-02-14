use std::io::Write;

use crate::{low_level::LowLevelGdtf, SerializationError};

// TODO further split up into serialize description and serialize into zip
pub fn serialize(llgdtf: &LowLevelGdtf) -> Result<Vec<u8>, SerializationError> {
    let mut description: String = concat!(r#"<?xml version="1.0" encoding="UTF-8"?>"#, "\n").into();
    quick_xml::se::to_writer(&mut description, &llgdtf)?;
    // println!("serialized with quick-xml from low level to XML:\n{description}");

    let mut out = Vec::<u8>::new();
    let buf = std::io::Cursor::new(&mut out);
    {
        let mut zip = zip::ZipWriter::new(buf);

        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("description.xml", options)?;
        zip.write_all(description.as_bytes())?;

        // Dropping the `ZipWriter` will have the same effect, but may silently fail
        zip.finish()?;
    }

    Ok(out)
}
