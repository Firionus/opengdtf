use thiserror::Error;

#[derive(Error, Debug)]
pub enum GdtfCompleteFailure {
    #[error("Invalid XML: {0}")]
    XmlError(#[from] roxmltree::Error),
}