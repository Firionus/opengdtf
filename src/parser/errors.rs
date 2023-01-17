use std::io;

use thiserror::Error;
use zip::result::ZipError;

/// An unrecoverable GDTF Parsing Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid XML: {0}")]
    InvalidXml(#[from] roxmltree::Error),
    #[error("root node 'GDTF' not found")]
    NoRootNode,
    #[error("zip error: {0}")]
    InvalidZip(#[from] ZipError),
    #[error("'description.xml' not found in GDTF zip archive: {0}")]
    DescriptionXmlMissing(ZipError),
    #[error("'description.xml' could not be read: {0}")]
    InvalidDescriptionXml(io::Error),
}
