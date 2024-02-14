use quick_xml::DeError;
use zip::result::ZipError;

#[derive(thiserror::Error, Debug)]
pub enum SerializationError {
    #[error("quick-xml could not serialize the low level GDTF representation: {0}")]
    QuickXmlError(#[from] DeError),
    #[error("zip error: {0}")]
    ZipError(#[from] ZipError),
    #[error("std::io::error: {0}")]
    StdIoError(#[from] std::io::Error),
}
