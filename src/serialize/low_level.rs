use std::io::Write;

use crate::{low_level::LowLevelGdtf, SerializationError};

impl LowLevelGdtf {
    pub fn serialize(&self) -> Result<Vec<u8>, SerializationError> {
        let description = self.serialize_description()?;

        let mut out = Vec::<u8>::new();
        let buf = std::io::Cursor::new(&mut out);
        {
            let mut zip = zip::ZipWriter::new(buf);

            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("description.xml", options)?;
            zip.write_all(description.as_bytes())?;

            // Dropping the `ZipWriter` will have the same effect, but may silently fail
            zip.finish()?;
        }

        Ok(out)
    }

    pub fn serialize_description(&self) -> Result<String, SerializationError> {
        let mut description: String =
            concat!(r#"<?xml version="1.0" encoding="UTF-8"?>"#, "\n").into();
        quick_xml::se::to_writer(&mut description, self)?;
        Ok(description)
    }
}
