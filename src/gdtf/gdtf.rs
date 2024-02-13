use derivative::Derivative;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{geometry::Geometry, name::Name};

use super::data_version::DataVersion;

#[derive(Debug, Serialize, Deserialize, PartialEq, Derivative)]
#[derivative(Default)]
pub struct Gdtf {
    #[derivative(Default(value = "DataVersion::V1_2"))]
    pub data_version: DataVersion,

    pub name: Name,
    pub short_name: String,
    pub long_name: String,
    pub manufacturer: String,
    pub description: String,

    pub fixture_type_id: Uuid,
    // Not implemented: Thumbnail, ThumbnailOffsetX, ThumbnailOffsetY
    pub ref_ft: Option<Uuid>,
    pub can_have_children: bool,

    geometries: Vec<Geometry>,
}
