/// domain errors go here
use thiserror::Error;

/// An unrecoverable GDTF Error.
#[derive(Error, Debug)]
pub enum Error {}
