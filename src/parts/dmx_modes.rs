use petgraph::{graph::NodeIndex, Graph, Directed};

#[derive(Debug)]
pub struct DmxMode {
    channel_count: u32,
    name: String,
    description: String,
    geometry: NodeIndex, // must be top-level

    dmx_channel_names: Vec<Vec<String>>,

    channel_function_graph: Graph<ChannelFunction, ModeMaster, Directed>,
}

#[derive(Debug)]
pub struct ChannelFunction {

}

#[derive(Debug)]
pub struct ModeMaster {
    
}