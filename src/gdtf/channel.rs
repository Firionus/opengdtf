use crate::{channel_offsets::ChannelOffsets, dmx_break::Break, name::Name};
use petgraph::graph::NodeIndex;

#[derive(Debug)]
pub struct Channel {
    pub name: Name,
    pub dmx_break: Break,
    /// only between 1 to 4 bytes are supported
    pub bytes: u8,
    pub offsets: ChannelOffsets,
    /// first one must always be the Raw DMX Channel Function
    pub channel_functions: Vec<NodeIndex>,
    pub default: u32,
}
