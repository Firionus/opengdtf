use std::{collections::HashMap, num::NonZeroU8};

use serde::{Deserialize, Serialize};

use crate::{dmx_address::DmxAddress, name::Name};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Geometry {
    name: Name,
    // TODO: model, position
    t: GeometryType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
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
        /// Needs to be a valid key into offsets.
        default_break: NonZeroU8,
        /// Maps DMX break to a corresponding DMX offset. Channels of the
        /// referenced geometry are instantiated at their DMX address added to
        /// this DMX offset.
        offsets: HashMap<NonZeroU8, DmxAddress>,
    },
}
