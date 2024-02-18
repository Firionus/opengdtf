use derivative::Derivative;
use getset::Getters;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    high_level::geometry::{find_geometry, find_geometry_mut, Geometry, GeometryType},
    GdtfError, Name,
};

use super::data_version::DataVersion;

#[derive(Debug, Serialize, Deserialize, PartialEq, Derivative, Clone, Getters)]
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

    #[getset(get = "pub")]
    geometries: Vec<Geometry>,
}

impl Gdtf {
    pub fn geometry(&self, name: &Name) -> Option<&Geometry> {
        find_geometry(&self.geometries, name)
    }

    pub fn top_level_geometry(&self, name: &Name) -> Option<&Geometry> {
        self.geometries.iter().find(|g| &g.name == name)
    }

    pub fn add_top_level_geometry(&mut self, geometry: Geometry) -> Result<(), GdtfError> {
        if let GeometryType::GeometryReference { .. } = geometry.t {
            Err(GdtfError::TopLevelGeometryReference())?
        };
        let geometry = self.check_unique_geometry_name(geometry)?;
        self.geometries.push(geometry);
        Ok(())
    }

    pub fn add_child_geometry(
        &mut self,
        parent: &Name,
        new_geometry: Geometry,
    ) -> Result<(), GdtfError> {
        let new_geometry = self.check_unique_geometry_name(new_geometry)?;

        if let GeometryType::GeometryReference {
            geometry,
            overwrite: default_break,
            offsets,
        } = &new_geometry.t
        {
            let referenced = self
                .top_level_geometry(geometry)
                .ok_or(GdtfError::UnknownTopLevelGeometryName(geometry.clone()))?;
            if let GeometryType::GeometryReference { .. } = referenced.t {
                Err(GdtfError::Unexpected(format!("There should be no top-level GeometryReference, yet there was one with name '{}'",referenced.name).into(),))?;
            }
            // TODO validate that channels of the referencd geometry only have breaks \
            // that are in offsets (or "overwrite", in which case we use the default break)
        }

        let parent = find_geometry_mut(&mut self.geometries, parent)
            .ok_or(GdtfError::UnknownGeometryName(parent.clone()))?;
        match parent.t {
            GeometryType::Geometry { ref mut children } => children.push(new_geometry),
            GeometryType::GeometryReference { .. } => {
                Err(GdtfError::ChildFreeGeometryType(parent.name.clone()))?
            }
        }
        Ok(())
    }

    fn check_unique_geometry_name(&self, g: Geometry) -> Result<Geometry, GdtfError> {
        if self.geometry(&g.name).is_some() {
            Err(GdtfError::DuplicateGeometryName(g))?
        } else {
            Ok(g)
        }
    }
}
