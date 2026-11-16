use core::panic;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use vulkano::image::ImageLayout;

use crate::rendering::graph::{
    graph::{GraphContext, GraphError},
    resource::{ResourceDescription, ResourceUsage},
};

pub type GraphNodeId = usize;

#[derive(Debug, Clone)]
pub enum BufferUsage {
    StorageRead,
    StorageWrite,
}

#[derive(Clone)]
pub enum NodeInputKind {
    Buffer(BufferUsage),
    Texture(ImageLayout),
    #[cfg(test)]
    TestKind,
}

/// The queue a node is intended to run on. Used to tell lanes apart in the graph dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueKind {
    #[default]
    Graphics,
    Compute,
    Transfer,
}

impl QueueKind {
    pub const ALL: [QueueKind; 3] = [QueueKind::Graphics, QueueKind::Compute, QueueKind::Transfer];

    pub fn name(&self) -> &'static str {
        match self {
            QueueKind::Graphics => "graphics",
            QueueKind::Compute => "compute",
            QueueKind::Transfer => "transfer",
        }
    }

    /// Mermaid `classDef` style for nodes of this queue kind.
    pub fn mermaid_style(&self) -> &'static str {
        match self {
            QueueKind::Graphics => "fill:#cfe8ff,stroke:#2b6cb0",
            QueueKind::Compute => "fill:#ffe3b3,stroke:#b7791f",
            QueueKind::Transfer => "fill:#d7f5d3,stroke:#2f855a",
        }
    }
}

#[derive(Clone)]
pub struct GraphNode {
    pub name: String,
    pub queue: QueueKind,
    pub node_type: Arc<dyn NodeType>,
    pub input_descriptions: HashMap<String, NodeInputKind>,
    pub resource_descriptions: HashMap<String, ResourceDescription>,
    pub resource_usages: HashMap<String, ResourceUsage>,
    pub outputs: HashSet<String>,
}

// Trait representing a node in the rendering graph.
// all things related to a single step or operation in the rendering graph.
pub trait NodeType {
    fn execute(&self, ctx: GraphContext) -> Result<(), GraphError>;
}

impl GraphNode {
    pub fn new(name: String, node_type: impl NodeType + 'static) -> Self {
        GraphNode {
            name,
            queue: QueueKind::default(),
            node_type: Arc::new(node_type),
            input_descriptions: HashMap::new(),
            resource_descriptions: HashMap::new(),
            resource_usages: HashMap::new(),
            outputs: HashSet::new(),
        }
    }

    pub fn with_queue(mut self, queue: QueueKind) -> Self {
        self.queue = queue;
        self
    }

    pub fn with_input(mut self, name: String, kind: NodeInputKind) -> Self {
        if self.input_descriptions.insert(name.clone(), kind).is_some() {
            log::error!(
                "GraphNode {} with the same input name already exists {}",
                self.name,
                name
            );
            panic!(
                "GraphNode {} with the same input name already exists {}",
                self.name, name
            );
        }
        self
    }

    pub fn with_resource(
        mut self,
        name: String,
        description: ResourceDescription,
        usage: ResourceUsage,
        is_output: bool,
    ) -> Self {
        let overridden_description = self.resource_descriptions.insert(name.clone(), description);
        let overridden_usage = self.resource_usages.insert(name.clone(), usage);

        if overridden_description.is_some() || overridden_usage.is_some() {
            log::error!(
                "GraphNode {} with the same resource name already exists {}",
                self.name,
                name
            );
            panic!(
                "GraphNode {} with the same resource name already exists {}",
                self.name, name
            );
        }
        if is_output {
            self.outputs.insert(name);
        }
        self
    }

    pub fn has_rendering(&self) -> bool {
        if self.queue == QueueKind::Graphics {
            self.resource_usages.values().any(|usage| match usage {
                ResourceUsage::ColorAttachment => true,
                ResourceUsage::DepthAttachment => true,
                _ => false,
            })
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct Connector {
    pub source_id: GraphNodeId,
    pub source_resource: String,
    pub destination_id: GraphNodeId,
    pub destination_input: String,
}

impl Connector {
    pub fn new(
        source_node_id: GraphNodeId,
        source_resource: String,
        destination_node_id: GraphNodeId,
        destination_input: String,
    ) -> Self {
        Connector {
            source_id: source_node_id,
            source_resource: source_resource,
            destination_id: destination_node_id,
            destination_input: destination_input,
        }
    }
}
