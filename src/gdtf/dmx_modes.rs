use petgraph::{graph::NodeIndex, Directed, Graph};

use crate::types::{dmx_break::Break, name::Name};

#[derive(Debug)]
pub struct DmxMode {
    pub name: Name,
    pub description: String,
    pub geometry: NodeIndex, // must be top-level

    pub channels: Vec<Channel>, // main channels (not template/subfixture)
    pub subfixtures: Vec<Subfixture>, // template/subfixture channels kept here

    pub channel_functions: ChannelFunctions, // non-instantiated channel functions
}

pub type ChannelFunctions = Graph<ChannelFunction, ModeMaster, Directed>;

#[derive(Debug)]
pub struct Channel {
    pub name: String,
    pub dmx_break: ChannelBreak,
    /// 0-based (1-based in GDTF), MSB to LSB
    pub offsets: Vec<u16>,
    pub channel_functions: Vec<NodeIndex>,
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
pub struct ChannelFunction {}

#[derive(Debug)]
pub struct ModeMaster {}
