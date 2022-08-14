use petgraph::{graph::NodeIndex, Directed, Graph};

#[derive(Debug)]
pub struct DmxMode {
    pub channel_count: u32,
    pub name: String,
    pub description: String,
    pub geometry: NodeIndex, // must be top-level

    pub dmx_channel_names: Vec<Vec<String>>,

    pub channel_function_graph: Graph<ChannelFunction, ModeMaster, Directed>,
}

#[derive(Debug)]
pub struct ChannelFunction {}

#[derive(Debug)]
pub struct ModeMaster {}
