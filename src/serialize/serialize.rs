use std::io::Write;

use quick_xml::DeError;
use zip::result::ZipError;

use crate::{low_level_gdtf::low_level_gdtf::LowLevelGdtf, Gdtf};

pub fn serialize_gdtf(gdtf: &Gdtf) -> Result<Vec<u8>, SerializationError> {
    let llgdtf = LowLevelGdtf {
        data_version: gdtf.data_version.to_owned(),
    };
    let mut description: String = r#"<?xml version="1.0" encoding="UTF-8"?>
"#
    .into();
    quick_xml::se::to_writer(&mut description, &llgdtf)?;

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

#[derive(thiserror::Error, Debug)]
pub enum SerializationError {
    #[error("quick-xml could not serialize the low level GDTF representation: {0}")]
    QuickXmlError(#[from] DeError),
    #[error("zip error: {0}")]
    ZipError(#[from] ZipError),
    #[error("std::io::error: {0}")]
    StdIoError(#[from] std::io::Error),
}
