use std::str::FromStr;

use derive_more::IntoIterator;
use petgraph::{graph::NodeIndex, Directed};
use thiserror::Error;

use crate::{checked_graph::CheckedGraph, dmx_break::Break, name::Name, Problem};

#[derive(Debug)]
pub struct DmxMode {
    pub name: Name,
    pub description: String,
    pub geometry: NodeIndex, // must be top-level

    pub channels: Vec<Channel>, // main channels (not template/subfixture)
    pub subfixtures: Vec<Subfixture>, // template/subfixture channels kept here

    pub channel_functions: ChannelFunctions,
}

/// ModeMaster Edges go from dependency to dependent channel function
pub type ChannelFunctions = CheckedGraph<ChannelFunction, ModeMaster, Directed>;

#[derive(Debug)]
pub struct Channel {
    pub name: Name,
    pub dmx_break: Break,
    /// only between 1 to 4 bytes are supported
    pub bytes: u8,
    /// 0-based (1-based in GDTF). MSB to LSB. Empty offsets might imply a virtual channel.
    pub offsets: ChannelOffsets,
    /// first one must always be the Raw DMX Channel Function
    pub channel_functions: Vec<NodeIndex>,
    pub initial_function: NodeIndex,
    pub default: u32,
}

pub fn chfs<'a>(
    channel_function_inds: &'a [NodeIndex],
    channel_functions: &'a ChannelFunctions,
) -> impl Iterator<Item = Result<(NodeIndex, &'a ChannelFunction), Problem>> + 'a {
    channel_function_inds.iter().map(|chf_ind| {
        let chf_ref = channel_functions
            .node_weight(*chf_ind)
            .ok_or_else(|| Problem::Unexpected("Invalid Channel Function Index".into()))?;
        Ok((*chf_ind, chf_ref))
    })
}

#[derive(Default, Debug, IntoIterator, derive_more::Deref, derive_more::DerefMut)]
pub struct ChannelOffsets(Vec<u16>);

#[derive(Debug, Error)]
pub enum OffsetError {
    #[error("Invalid Offset Format")]
    Invalid,
    #[error("DXM address offsets must be between 1 and 512")]
    OutsideRange,
}

impl FromStr for ChannelOffsets {
    type Err = OffsetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut out = Self::default();

        if s == "None" {
            return Ok(out);
        };

        for s in s.split(',') {
            let u: u16 = s.parse().map_err(|_| OffsetError::Invalid)?;
            if (1..=512).contains(&u) {
                out.0.push(u - 1);
            } else {
                return Err(OffsetError::OutsideRange);
            }
        }

        Ok(out)
    }
}

impl FromIterator<u16> for ChannelOffsets {
    fn from_iter<I: IntoIterator<Item = u16>>(iter: I) -> Self {
        Self(Vec::from_iter(iter))
    }
}

#[derive(Debug)]
pub struct Subfixture {
    pub name: Name,
    pub channels: Vec<Channel>,
    pub geometry: NodeIndex,
}

#[derive(Debug)]
pub struct ChannelFunction {
    pub name: Name,
    pub geometry: NodeIndex,
    pub attr: String, // TODO replace by index down the line, I guess
    pub original_attr: String,
    pub dmx_from: u32, // max supported DMX channels per GDTF channel is 4
    pub dmx_to: u32,
    pub phys_from: f64,
    pub phys_to: f64,
    pub default: u32,
}

#[derive(Debug)]
pub struct ModeMaster {
    pub from: u32,
    pub to: u32,
}
