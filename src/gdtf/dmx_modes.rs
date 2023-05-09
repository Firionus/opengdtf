use petgraph::{graph::NodeIndex, Directed, Graph};

use crate::{dmx_break::Break, name::Name};

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
pub type ChannelFunctions = Graph<ChannelFunction, ModeMaster, Directed>;

#[derive(Debug)]
pub struct Channel {
    pub name: Name,
    pub dmx_break: ChannelBreak,
    /// only between 1 to 4 bytes are supported
    pub bytes: u8,
    /// 0-based (1-based in GDTF). MSB to LSB. Empty offsets might imply a virtual channel.
    pub offsets: Vec<u16>,
    /// first one must always be the Raw DMX Channel Function
    pub channel_functions: Vec<NodeIndex>,
    pub default: u32,
}

#[derive(Debug)]
pub struct Subfixture {
    name: String,
    channels: Vec<Channel>,
    geometry: NodeIndex,
}

#[derive(Debug)]
pub enum ChannelBreak {
    Overwrite,
    Break(Break),
}

impl Default for ChannelBreak {
    fn default() -> Self {
        ChannelBreak::Break(Break::default())
    }
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
