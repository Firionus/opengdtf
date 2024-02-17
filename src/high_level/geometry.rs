use std::{collections::HashMap, num::NonZeroU8};

use serde::{Deserialize, Serialize};

use crate::{DmxAddress, Name};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Geometry {
    pub name: Name,
    // TODO: model, position
    pub t: GeometryType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum GeometryType {
    Geometry {
        children: Vec<Geometry>,
    },
    GeometryReference {
        /// Referenced top-level geometry. The referenced geometry must not be
        /// the root of this reference to avoid endless geometry trees.
        geometry: Name,
        /// Used when a DMX channel of the referenced geometry specifies
        /// "Overwrite" as its DMX Break.
        ///
        /// Break can also exist in offsets with a different DmxAddress.
        overwrite: (NonZeroU8, DmxAddress),
        /// Maps DMX break to a corresponding DMX offset. Channels of the
        /// referenced geometry are instantiated at their DMX address added to
        /// this DMX offset.
        offsets: HashMap<NonZeroU8, DmxAddress>,
    },
}

pub fn find_geometry<'a>(collection: &'a [Geometry], name: &Name) -> Option<&'a Geometry> {
    for g in collection.iter() {
        if &g.name == name {
            return Some(g);
        }
        match &g.t {
            GeometryType::Geometry { children } => match find_geometry(children, name) {
                Some(c) => return Some(c),
                None => continue,
            },
            GeometryType::GeometryReference { .. } => continue,
        }
    }
    None
}
pub fn find_geometry_mut<'a>(
    collection: &'a mut [Geometry],
    name: &Name,
) -> Option<&'a mut Geometry> {
    for g in collection.iter_mut() {
        if &g.name == name {
            return Some(g);
        }
        match &mut g.t {
            GeometryType::Geometry { children } => match find_geometry_mut(children, name) {
                Some(c) => return Some(c),
                None => continue,
            },
            GeometryType::GeometryReference { .. } => continue,
        }
    }
    None
}
