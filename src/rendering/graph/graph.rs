use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use vulkano::pipeline::graphics::viewport::{Scissor, Viewport};
use vulkano::{
    image::ImageLayout,
    sync::{AccessFlags, BufferMemoryBarrier, DependencyInfo, ImageMemoryBarrier, PipelineStages},
};

use crate::rendering::command_buffer::{
    ChaosClearValue, ChaosCommandBuffer, ChaosRenderAttachment, ChaosRenderAttachmentLoadOp,
};
use crate::rendering::errors::RenderError;
use crate::rendering::graph::resource::{ResourceDescription, ResourceUsage};
use crate::rendering::helpers::{scissor, viewport, viewport_with_offset};
use crate::rendering::{
    context::ChaosRenderContext,
    graph::{
        node::{BufferUsage, Connector, GraphNode, GraphNodeId, NodeInputKind, QueueKind},
        resource::{Resource, ResourceHandle, TextureSizeType, layout_scope},
    },
    texture::ChaosTexture,
};

pub type ResourceId = usize;

/// A problem with how the graph is put together. These are found when connecting nodes
/// or when the graph is compiled (`sort`), never while it executes. Compilation gathers
/// every problem it finds, so a graph with three wiring mistakes reports all three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphBuildError {
    SourceNodeNotFound {
        node_id: GraphNodeId,
    },
    DestinationNodeNotFound {
        node_id: GraphNodeId,
    },
    DestinationInputNotFound {
        node_id: GraphNodeId,
        input_name: String,
    },
    SourceResourceNotFound {
        node_id: GraphNodeId,
        node_name: String,
        resource_name: String,
    },
    ConnectionBetweenSameNode {
        node_id: GraphNodeId,
    },
    ConnectionAlreadyExists {
        destination_id: GraphNodeId,
        destination_input: String,
    },
    InputNotConnected {
        node_name: String,
        input_name: String,
    },
    /// An input expects a different kind of resource (texture/buffer) than the one
    /// connected to it.
    InputKindMismatch {
        node_name: String,
        input_name: String,
        expected: &'static str,
        source_node_name: String,
        source_resource: String,
    },
    /// A texture input asks for a layout the graph can't transition to.
    InvalidInputLayout {
        node_name: String,
        input_name: String,
        layout: ImageLayout,
    },
    ImportedResourceSettingMismatch {
        resource_name: String,
    },
    CircularConnection(Vec<String>),
    InvalidClearValue {
        node_name: String,
        resource_name: String,
        usage: String,
    },
    LoadBeforeWrite {
        node_name: String,
        resource_name: String,
    },
}

/// Errors that can happen while the graph runs.
#[derive(Debug)]
pub enum GraphError {
    /// The graph failed to compile. Holds every problem that was found.
    Invalid(Vec<GraphBuildError>),
    /// A node asked its context for a resource or input name it doesn't have.
    NodeResourceNotFound { resource_name: String },
    /// A node's `execute` failed.
    NodeFailed {
        node_name: String,
        source: Box<GraphError>,
    },
    /// Creating or using a GPU resource failed.
    Render(RenderError),
    /// Free-form error for node implementations.
    Custom(String),
    /// Unknown graph destination
    UnknownDestination(String, Vec<String>),
}

impl From<RenderError> for GraphError {
    fn from(value: RenderError) -> Self {
        GraphError::Render(value)
    }
}

impl fmt::Display for GraphBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphBuildError::SourceNodeNotFound { node_id } => {
                write!(f, "source node {node_id} doesn't exist")
            }
            GraphBuildError::DestinationNodeNotFound { node_id } => {
                write!(f, "destination node {node_id} doesn't exist")
            }
            GraphBuildError::DestinationInputNotFound {
                node_id,
                input_name,
            } => write!(f, "node {node_id} has no input '{input_name}'"),
            GraphBuildError::SourceResourceNotFound {
                node_name,
                resource_name,
                ..
            } => write!(f, "node '{node_name}' has no output '{resource_name}'"),
            GraphBuildError::ConnectionBetweenSameNode { node_id } => {
                write!(f, "node {node_id} can't be connected to itself")
            }
            GraphBuildError::ConnectionAlreadyExists {
                destination_id,
                destination_input,
            } => write!(
                f,
                "input '{destination_input}' on node {destination_id} is already connected"
            ),
            GraphBuildError::InputNotConnected {
                node_name,
                input_name,
            } => write!(
                f,
                "input '{input_name}' on node '{node_name}' is not connected"
            ),
            GraphBuildError::InputKindMismatch {
                node_name,
                input_name,
                expected,
                source_node_name,
                source_resource,
            } => write!(
                f,
                "input '{input_name}' on node '{node_name}' expects a {expected}, but \
                 '{source_resource}' on node '{source_node_name}' isn't one"
            ),
            GraphBuildError::InvalidInputLayout {
                node_name,
                input_name,
                layout,
            } => write!(
                f,
                "input '{input_name}' on node '{node_name}' asks for unsupported layout {layout:?}"
            ),
            GraphBuildError::CircularConnection(nodes) => {
                write!(f, "circular connection between nodes: {}", nodes.join(", "))
            }
            GraphBuildError::ImportedResourceSettingMismatch { resource_name } => write!(
                f,
                "imported resource '{resource_name}' is imported twice with mismatching settings"
            ),
            GraphBuildError::InvalidClearValue {
                node_name,
                resource_name,
                usage,
            } => write!(
                f,
                "resource '{resource_name}' on node '{node_name}' has an invalid clear value '{usage}'"
            ),
            GraphBuildError::LoadBeforeWrite {
                node_name,
                resource_name,
            } => write!(
                f,
                "resource '{resource_name}' in node '{node_name}' is loaded before it has been written to"
            ),
        }
    }
}

impl Error for GraphBuildError {}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::Invalid(problems) => {
                write!(f, "render graph is invalid:")?;
                for problem in problems {
                    write!(f, "\n  - {problem}")?;
                }
                Ok(())
            }
            GraphError::NodeResourceNotFound { resource_name } => {
                write!(f, "node has no resource or input named '{resource_name}'")
            }
            GraphError::NodeFailed { node_name, .. } => write!(f, "node '{node_name}' failed"),
            GraphError::Render(_) => write!(f, "rendering error in graph"),
            GraphError::Custom(message) => write!(f, "{message}"),
            GraphError::UnknownDestination(name, available_names) => {
                write!(f, "{name} unknown, available names are {available_names:?}")
            }
        }
    }
}

impl Error for GraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            GraphError::NodeFailed { source, .. } => Some(source.as_ref()),
            GraphError::Render(error) => Some(error),
            GraphError::Invalid(_)
            | GraphError::NodeResourceNotFound { .. }
            | GraphError::Custom(_)
            | GraphError::UnknownDestination(_, _) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Job {
    BarrierJob {
        resource_id: ResourceId,
        image_layout: ImageLayout,
    },
    BufferBarrierJob {
        resource_id: ResourceId,
        usage: BufferUsage,
    },
    // inputs are keyed by the node's own input name, outputs by the node's resource name.
    // Only ids are stored since the handles are replaced whenever the resources are recreated.
    NodeExecutionJob {
        graph_node_id: GraphNodeId,
        inputs: Vec<(String, ResourceId)>,
        outputs: Vec<(String, ResourceId)>,
    },
}

/// The resources a node can reach by name, resolved by the graph right before the node executes.
#[derive(Debug)]
pub struct NodeResources {
    entries: Vec<(String, Arc<ResourceHandle>)>,
}

impl NodeResources {
    pub fn new(entries: Vec<(String, Arc<ResourceHandle>)>) -> Self {
        Self { entries }
    }

    pub fn get(&self, name: &str) -> Result<&Arc<ResourceHandle>, GraphError> {
        self.entries
            .iter()
            .find(|(entry_name, _)| entry_name == name)
            .map(|(_, handle)| handle)
            .ok_or_else(|| GraphError::NodeResourceNotFound {
                resource_name: name.to_string(),
            })
    }
}
pub(crate) struct GraphContextNodeReport {
    pub node_name: String,
    pub message: String,
}

pub struct GraphContext<'a> {
    pub node_name: String,
    pub render_size: [u32; 2],
    pub display_size: [u32; 2],
    pub inputs: NodeResources,
    pub node_resources: NodeResources,
    pub command_buffer: &'a mut ChaosCommandBuffer,
    reports: &'a mut Vec<GraphContextNodeReport>,
}

impl GraphContext<'_> {
    pub fn input(&self, name: &str) -> Result<&Arc<ResourceHandle>, GraphError> {
        self.inputs.get(name)
    }

    pub fn resource(&self, name: &str) -> Result<&Arc<ResourceHandle>, GraphError> {
        self.node_resources.get(name)
    }

    pub fn report(&mut self, message: impl Into<String>) {
        self.reports.push(GraphContextNodeReport {
            node_name: self.node_name.clone(),
            message: message.into(),
        });
    }
}

/**
 * Respresents a rendering graph, which is a directed acyclic graph (DAG) of nodes that represent rendering operations.
 * Each node can have multiple inputs and outputs, which are connected to other nodes in the graph.
 *
 * The graph is responsible for managing the execution order of the nodes, ensuring that all dependencies are met before executing a node.
 * It is also reponsible for managing the resources used by the nodes, ensuring that they are created and destroyed at the appropriate times.
 */
pub struct Graph {
    current_node_id: GraphNodeId,
    current_resource_id: ResourceId,
    nodes: HashMap<GraphNodeId, Arc<GraphNode>>,
    connections: Vec<Connector>,
    resources: HashMap<ResourceId, Arc<ResourceHandle>>,
    resource_lookup: HashMap<(GraphNodeId, String), ResourceId>,
    layouts: HashMap<ResourceId, ImageLayout>,
    imported_resources: HashMap<String, ResourceId>,
    // jobs are sorted by levels for multithreaded rendering and async compute
    jobs: Vec<Vec<Job>>,
    sort_jobs: bool,
    recreate_resources: bool,
    render_size: [u32; 2],
    display_size: [u32; 2],
    viewport_override: Option<Viewport>,
    scissor_override: Option<Scissor>,
    render_size_changed: bool,
    display_size_changed: bool,
    reported_errors: HashSet<(String, String)>,
}

impl Graph {
    pub fn new() -> Self {
        // have the render_size very small so we can clearly see if the render size is not set
        let render_size = [100, 100];
        Graph {
            current_node_id: 0,
            current_resource_id: 0,
            nodes: HashMap::new(),
            connections: Vec::new(),
            resources: HashMap::new(),
            resource_lookup: HashMap::new(),
            layouts: HashMap::new(),
            imported_resources: HashMap::new(),
            jobs: Vec::new(),
            sort_jobs: true,
            recreate_resources: true,
            render_size,
            display_size: render_size,
            viewport_override: None,
            scissor_override: None,
            render_size_changed: true,
            display_size_changed: true,
            reported_errors: HashSet::new(),
        }
    }

    fn graph_context<'a>(
        &self,
        node_id: GraphNodeId,
        command_buffer: &'a mut ChaosCommandBuffer,
        inputs: &[(String, ResourceId)],
        resources: &[(String, ResourceId)],
        reports: &'a mut Vec<GraphContextNodeReport>,
    ) -> GraphContext<'a> {
        let node_name = match self.nodes.get(&node_id) {
            Some(node) => node.name.clone(),
            None => unreachable!(),
        };
        GraphContext {
            node_name,
            render_size: self.render_size,
            display_size: self.display_size,
            inputs: self.node_resources(node_id, inputs),
            node_resources: self.node_resources(node_id, resources),
            command_buffer,
            reports,
        }
    }

    /// Swaps the resource ids resolved by `sort` for the current resource handles.
    /// The ids come from `sort`, which only hands out ids that exist.
    fn node_resources(
        &self,
        node_id: GraphNodeId,
        bindings: &[(String, ResourceId)],
    ) -> NodeResources {
        let entries = bindings
            .iter()
            .map(|(name, resource_id)| {
                let handle = self.resources.get(resource_id).unwrap_or_else(|| {
                    panic!("sort resolved '{name}' on node {node_id} to a missing resource")
                });
                (name.clone(), handle.clone())
            })
            .collect();
        NodeResources::new(entries)
    }

    pub fn set_render_size(&mut self, render_size: [u32; 2]) {
        if self.render_size == render_size {
            return;
        }
        self.render_size = render_size;
        self.recreate_resources = true;
        self.render_size_changed = true;
    }

    pub fn set_display_size(&mut self, display_size: [u32; 2]) {
        if self.display_size == display_size {
            return;
        }
        self.display_size = display_size;
        self.recreate_resources = true;
        self.display_size_changed = true;
    }

    pub fn set_viewport_override(&mut self, size: [f32; 2], offset: Option<[f32; 2]>) {
        self.viewport_override = Some(match offset {
            Some(o) => viewport_with_offset(size, o),
            None => viewport(size),
        });
    }

    pub fn remove_viewport_override(&mut self) {
        self.viewport_override = None;
    }

    pub fn set_scissor_override(&mut self, extent: [u32; 2], offset: [u32; 2]) {
        self.scissor_override = Some(scissor(extent, offset));
    }

    pub fn remove_scissor(&mut self) {
        self.scissor_override = None;
    }

    // nodes are moved into the graph so they don't get updated outside of the graph
    pub fn add_node(&mut self, node: GraphNode) -> Result<GraphNodeId, GraphBuildError> {
        let node_id = self.current_node_id;
        self.current_node_id += 1;
        self.sort_jobs = true;

        for (name, resource_description) in node.resource_descriptions.iter() {
            let mut resource_id = self.current_resource_id;
            match resource_description {
                ResourceDescription::Imported {
                    resource_name,
                    setting,
                } => match self.imported_resources.get(resource_name) {
                    Some(shared_resource_id) => {
                        let new_setting = setting;
                        // check if the settings are the same for the stored imported resource
                        match &self
                            .resources
                            .get(shared_resource_id)
                            .unwrap()
                            .clone()
                            .description
                        {
                            ResourceDescription::Imported { setting, .. } => {
                                if setting.initial_layout != new_setting.initial_layout
                                    || setting.end_layout != new_setting.end_layout
                                {
                                    return Err(GraphBuildError::ImportedResourceSettingMismatch {
                                        resource_name: resource_name.clone(),
                                    });
                                }
                            }
                            _ => (), // this will never happen
                        }
                        resource_id = *shared_resource_id;
                    }
                    None => {
                        self.imported_resources
                            .insert(resource_name.clone(), resource_id);
                        self.current_resource_id += 1;

                        let resource_handle = ResourceHandle::new(resource_description, name);
                        self.resources
                            .insert(resource_id, Arc::new(resource_handle));
                    }
                },
                _ => {
                    self.current_resource_id += 1;
                    let resource_handle = ResourceHandle::new(resource_description, name);

                    self.resources
                        .insert(resource_id, Arc::new(resource_handle));
                }
            }

            self.resource_lookup
                .insert((node_id, name.clone()), resource_id);

            self.recreate_resources = true;
        }

        self.nodes.insert(node_id, Arc::new(node));

        Ok(node_id)
    }

    pub fn connect(
        &mut self,
        source: GraphNodeId,
        source_resource: String,
        destination: GraphNodeId,
        destination_input: String,
    ) -> Result<(), GraphBuildError> {
        if source == destination {
            return Err(GraphBuildError::ConnectionBetweenSameNode { node_id: source });
        }

        // check if there is already a connection to the destination and input
        if self.connections.iter().any(|connection| {
            connection.destination_id == destination
                && connection.destination_input == destination_input
        }) {
            return Err(GraphBuildError::ConnectionAlreadyExists {
                destination_id: destination,
                destination_input: destination_input.clone(),
            });
        }

        // check if the source exists
        match self.nodes.get(&source) {
            Some(source_node) => {
                // source node needs to contain the specified resource
                if !source_node.outputs.contains(&source_resource) {
                    return Err(GraphBuildError::SourceResourceNotFound {
                        node_id: source,
                        node_name: source_node.name.clone(),
                        resource_name: source_resource,
                    });
                }

                // check if the destination node exists
                match self.nodes.get(&destination) {
                    Some(destination_node) => {
                        // destination node needs to contain the specified input
                        if !destination_node
                            .input_descriptions
                            .contains_key(&destination_input)
                        {
                            return Err(GraphBuildError::DestinationInputNotFound {
                                node_id: destination,
                                input_name: destination_input,
                            });
                        }

                        self.connections.push(Connector::new(
                            source,
                            source_resource,
                            destination,
                            destination_input,
                        ));
                        self.sort_jobs = true;

                        return Ok(());
                    }
                    None => {
                        return Err(GraphBuildError::DestinationNodeNotFound {
                            node_id: destination,
                        });
                    }
                }
            }
            None => {
                return Err(GraphBuildError::SourceNodeNotFound { node_id: source });
            }
        }
    }

    /// Checks everything that can be checked without running the graph and returns every
    /// problem found. Cycles are detected separately in `sort`, since they fall out of the
    /// level computation.
    fn validate(&self) -> Vec<GraphBuildError> {
        let mut problems = Vec::new();

        let mut node_ids: Vec<&GraphNodeId> = self.nodes.keys().collect();
        node_ids.sort_unstable();
        for &node_id in node_ids {
            let node = &self.nodes[&node_id];
            let mut input_names: Vec<&String> = node.input_descriptions.keys().collect();
            input_names.sort_unstable();
            for input_name in input_names {
                let kind = &node.input_descriptions[input_name];

                if let NodeInputKind::Texture(layout) = kind {
                    if *layout == ImageLayout::Undefined || layout_scope(*layout).is_none() {
                        problems.push(GraphBuildError::InvalidInputLayout {
                            node_name: node.name.clone(),
                            input_name: input_name.clone(),
                            layout: *layout,
                        });
                    }
                }

                let Some(connection) = self
                    .connections
                    .iter()
                    .find(|c| c.destination_id == node_id && c.destination_input == *input_name)
                else {
                    problems.push(GraphBuildError::InputNotConnected {
                        node_name: node.name.clone(),
                        input_name: input_name.clone(),
                    });
                    continue;
                };

                let source_node = &self.nodes[&connection.source_id];
                let Some(description) = source_node
                    .resource_descriptions
                    .get(&connection.source_resource)
                else {
                    problems.push(GraphBuildError::SourceResourceNotFound {
                        node_id: connection.source_id,
                        node_name: source_node.name.clone(),
                        resource_name: connection.source_resource.clone(),
                    });
                    continue;
                };

                if let Some(expected) = kind_mismatch(kind, description) {
                    problems.push(GraphBuildError::InputKindMismatch {
                        node_name: node.name.clone(),
                        input_name: input_name.clone(),
                        expected,
                        source_node_name: source_node.name.clone(),
                        source_resource: connection.source_resource.clone(),
                    });
                }
            }

            let mut usages: Vec<_> = node.resource_usages.iter().collect();
            usages.sort_by(|a, b| a.0.cmp(b.0));

            for (resource_name, usage) in usages {
                match &usage {
                    ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Clear(v)) => {
                        if !matches!(
                            v,
                            ChaosClearValue::Float(_)
                                | ChaosClearValue::Int(_)
                                | ChaosClearValue::Uint(_)
                        ) {
                            problems.push(GraphBuildError::InvalidClearValue {
                                node_name: node.name.clone(),
                                resource_name: resource_name.clone(),
                                usage: format!("{:?}", usage),
                            });
                        }
                    }
                    ResourceUsage::DepthAttachment(ChaosRenderAttachmentLoadOp::Clear(v)) => {
                        if !matches!(
                            v,
                            ChaosClearValue::Depth(_) | ChaosClearValue::DepthStencil(_, _)
                        ) {
                            problems.push(GraphBuildError::InvalidClearValue {
                                node_name: node.name.clone(),
                                resource_name: resource_name.clone(),
                                usage: format!("{:?}", usage),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        problems
    }

    fn validate_load_ops(&self, levels: &[Vec<GraphNodeId>]) -> Vec<GraphBuildError> {
        let mut error_report = vec![];
        let mut written: HashSet<ResourceId> = self
            .imported_resources
            .iter()
            .filter_map(|(_, resource_id)| match self.resources.get(resource_id) {
                Some(resource) => match resource.description {
                    ResourceDescription::Imported { ref setting, .. } => {
                        if setting.initial_layout != ImageLayout::Undefined {
                            Some(*resource_id)
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                None => None,
            })
            .collect::<HashSet<ResourceId>>();

        for nodes in levels {
            for node_id in nodes {
                if let Some(node) = self.nodes.get(node_id) {
                    let mut usages: Vec<_> = node.resource_usages.iter().collect();
                    usages.sort_by(|a, b| a.0.cmp(b.0));

                    for (resource_name, usage) in usages {
                        let resource_id = self.resource_lookup[&(*node_id, resource_name.clone())];

                        let loads = matches!(
                            usage,
                            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Load)
                                | ResourceUsage::DepthAttachment(ChaosRenderAttachmentLoadOp::Load)
                        );
                        if loads && !written.contains(&resource_id) {
                            error_report.push(GraphBuildError::LoadBeforeWrite {
                                node_name: node.name.clone(),
                                resource_name: resource_name.clone(),
                            });
                        }
                        if !loads {
                            written.insert(resource_id);
                        }
                    }
                }
            }
        }
        error_report
    }

    /// Compiles the graph into jobs. Fails with every problem `validate` and the cycle check
    /// found; once it succeeds, executing the jobs can assume a valid graph.
    fn sort(&mut self) -> Result<(), GraphError> {
        self.reported_errors.clear();
        let mut problems = self.validate();

        let (parents, children) = self.create_hierarchy();
        let levels = self.compute_levels(parents, children);
        let mut load_op_problems = self.validate_load_ops(&levels);
        problems.append(&mut load_op_problems);

        let scheduled: HashSet<GraphNodeId> = levels.iter().flatten().copied().collect();
        if scheduled.len() != self.nodes.len() {
            let mut erroneous_nodes: Vec<(GraphNodeId, String)> = self
                .nodes
                .iter()
                .filter(|(node_id, _)| !scheduled.contains(node_id))
                .map(|(&node_id, node)| (node_id, node.name.clone()))
                .collect();
            erroneous_nodes.sort();
            problems.push(GraphBuildError::CircularConnection(
                erroneous_nodes.into_iter().map(|(_, name)| name).collect(),
            ));
        }

        if !problems.is_empty() {
            return Err(GraphError::Invalid(problems));
        }

        self.jobs.clear();
        let inputs = self.resolve_inputs();

        // check where the imported resources are used in the levels, marking the start and end levels
        let mut imported_resource_ids = self
            .imported_resources
            .iter()
            .map(|(_, resource_id)| *resource_id)
            .collect::<Vec<ResourceId>>();
        imported_resource_ids.sort();

        // the places that use the imported resource last, so we can know when to add barriers
        let mut last_usage_level: HashMap<ResourceId, usize> = HashMap::new();

        for (level_index, level) in levels.iter().enumerate() {
            let mut level_jobs: Vec<Job> = vec![];
            // check if the level is the first usage of an imported texture
            for n_id in level {
                let node_id = *n_id;
                let node = &self.nodes[&node_id];
                let mut node_inputs: Vec<(String, ResourceId)> = Vec::new();
                // add all barriers for the current node
                for (input_name, kind) in node.input_descriptions.iter() {
                    let resource_id = *inputs
                        .get(&(node_id, input_name.clone()))
                        .expect("validation ensured this input is connected");
                    last_usage_level.insert(resource_id, level_index);

                    // add a barrier
                    match kind {
                        NodeInputKind::Texture(layout) => level_jobs.push(Job::BarrierJob {
                            resource_id,
                            image_layout: *layout,
                        }),
                        NodeInputKind::Buffer(usage) => level_jobs.push(Job::BufferBarrierJob {
                            resource_id,
                            usage: usage.clone(),
                        }),
                        #[cfg(test)]
                        NodeInputKind::TestKind => {}
                    }
                    node_inputs.push((input_name.clone(), resource_id));
                }

                for (name, usage) in node.resource_usages.iter() {
                    let resource_id = *self
                        .resource_lookup
                        .get(&(node_id, name.clone()))
                        .expect("add_node registers every resource the node declares");

                    last_usage_level.insert(resource_id, level_index);

                    // add a barrier
                    level_jobs.push(Job::BarrierJob {
                        resource_id,
                        image_layout: usage.layout(),
                    });
                }

                // every resource the node owns, so it can write to them
                let mut node_outputs: Vec<(String, ResourceId)> = node
                    .resource_descriptions
                    .keys()
                    .map(|name| {
                        let resource_id = *self
                            .resource_lookup
                            .get(&(node_id, name.clone()))
                            .expect("add_node registers every resource the node declares");
                        (name.clone(), resource_id)
                    })
                    .collect();

                // the maps iterate in arbitrary order, keep the jobs stable
                node_inputs.sort();
                node_outputs.sort();

                level_jobs.push(Job::NodeExecutionJob {
                    graph_node_id: node_id,
                    inputs: node_inputs,
                    outputs: node_outputs,
                });
            }

            self.jobs.push(level_jobs);
        }

        // now insert the imported resource barriers when the resource is not used anymore
        for resource_id in imported_resource_ids {
            let layout = match &self.resources.get(&resource_id).unwrap().description {
                ResourceDescription::Imported { setting, .. } => setting.end_layout,
                _ => unreachable!(),
            };
            if let Some(&level_id) = last_usage_level.get(&resource_id) {
                self.jobs[level_id].push(Job::BarrierJob {
                    resource_id,
                    image_layout: layout,
                });
            }
        }

        self.sort_jobs = false;
        Ok(())
    }

    pub fn render(
        &mut self,
        destinations: &HashMap<String, Arc<ChaosTexture>>,
        command_buffer: &mut ChaosCommandBuffer,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), GraphError> {
        // important to create resources before jobs
        if self.recreate_resources {
            self.recreate_resources(render_context)?;
        }

        if self.sort_jobs {
            self.sort()?;
        }

        // bind imported textures for this frame
        self.bind_imports(destinations)?;

        let mut reports = Vec::<GraphContextNodeReport>::new();

        for level in &self.jobs {
            for job in level {
                match job {
                    Job::BarrierJob {
                        resource_id,
                        image_layout,
                    } => Self::add_resource_barrier(
                        *image_layout,
                        *resource_id,
                        &mut self.layouts,
                        &self.resources,
                        command_buffer,
                    ),
                    Job::BufferBarrierJob { resource_id, usage } => Self::add_buffer_barrier(
                        usage,
                        *resource_id,
                        &self.resources,
                        command_buffer,
                    )?,
                    Job::NodeExecutionJob {
                        graph_node_id,
                        inputs,
                        outputs,
                    } => {
                        if let Some(node) = self.nodes.get(graph_node_id) {
                            self.execute_node(
                                command_buffer,
                                graph_node_id,
                                node,
                                inputs,
                                outputs,
                                &mut reports,
                            )?;
                        }
                    }
                }
            }
        }

        // go over the reports and log errors
        for report in reports {
            let name = report.node_name;
            let message = report.message;
            let value = (name.clone(), message.clone());
            if self.reported_errors.insert(value) {
                log::warn!("Execution failed for graph node '{name}': '{message}'");
            }
        }

        Ok(())
    }

    fn bind_imports(
        &mut self,
        destinations: &HashMap<String, Arc<ChaosTexture>>,
    ) -> Result<(), GraphError> {
        for (name, resource_id) in &self.imported_resources {
            match destinations.get(name) {
                Some(texture) => {
                    let handle = self.resources.get(&resource_id).unwrap();
                    let initial_layout = match &handle.description {
                        ResourceDescription::Imported { setting, .. } => setting.initial_layout,
                        _ => unreachable!(),
                    };
                    self.layouts.insert(*resource_id, initial_layout);
                    let resource =
                        ResourceHandle::new_with_texture(&handle.description, name, &texture);
                    self.resources.insert(*resource_id, Arc::new(resource));
                }
                None => {
                    let mut names: Vec<String> = destinations
                        .keys()
                        .map(|n| n.clone())
                        .collect::<Vec<String>>();
                    names.sort();
                    return Err(GraphError::UnknownDestination(name.clone(), names));
                }
            }
        }
        Ok(())
    }

    fn recreate_resources(
        &mut self,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), GraphError> {
        for (resource_id, resource_handle) in self.resources.iter_mut() {
            let dependant_on_render_size = match &resource_handle.description {
                ResourceDescription::Texture(desc) => {
                    desc.dimension_calculations.iter().any(|dim| {
                        *dim == TextureSizeType::PercentageOfRenderWidth
                            || *dim == TextureSizeType::PercentageOfRenderHeight
                    })
                }
                _ => false,
            };

            let dependant_on_display_size = match &resource_handle.description {
                ResourceDescription::Texture(desc) => {
                    desc.dimension_calculations.iter().any(|dim| {
                        *dim == TextureSizeType::PercentageOfDisplayWidth
                            || *dim == TextureSizeType::PercentageOfDisplayHeight
                    })
                }
                _ => false,
            };

            let recreate_resource = resource_handle.resource.is_none()
                || (self.render_size_changed && dependant_on_render_size)
                || (self.display_size_changed && dependant_on_display_size);

            if recreate_resource {
                let mut new_handle =
                    ResourceHandle::new(&resource_handle.description, &resource_handle.name);
                new_handle.recreate_resource(
                    self.display_size,
                    self.render_size,
                    render_context.clone(),
                )?;
                *resource_handle = Arc::new(new_handle);
                self.layouts.insert(*resource_id, ImageLayout::Undefined);
            }
        }
        self.recreate_resources = false;
        self.render_size_changed = false;
        self.display_size_changed = false;
        Ok(())
    }

    fn execute_node(
        &self,
        command_buffer: &mut ChaosCommandBuffer,
        graph_node_id: &GraphNodeId,
        node: &Arc<GraphNode>,
        inputs: &[(String, ResourceId)],
        outputs: &[(String, ResourceId)],
        report: &mut Vec<GraphContextNodeReport>,
    ) -> Result<(), GraphError> {
        let has_rendering = node.has_rendering();
        if has_rendering {
            let mut color_attachments: Vec<Option<Arc<ChaosRenderAttachment>>> = vec![];
            let mut depth_attachment: Option<Arc<ChaosRenderAttachment>> = None;

            for (name, id) in outputs {
                let usage = node
                    .resource_usages
                    .get(name)
                    .expect("with_resource registers a usage for every resource");

                let resource_handle = self
                    .resources
                    .get(id)
                    .expect("sort registers every resource");
                match usage {
                    ResourceUsage::ColorAttachment(loadop) => {
                        color_attachments.push(Some(resource_handle.as_attachment(
                            loadop.clone(),
                            usage.layout(),
                            true,
                        )));
                    }
                    ResourceUsage::DepthAttachment(loadop) => {
                        if depth_attachment.is_none() {
                            depth_attachment = Some(resource_handle.as_attachment(
                                loadop.clone(),
                                usage.layout(),
                                true,
                            ));
                        } else {
                            panic!("More than one depth attachment used in rendering");
                        }
                    }
                    _ => {}
                };
            }

            command_buffer.begin_rendering(
                color_attachments.as_slice(),
                depth_attachment,
                None,
                None,
                None,
            );
            match &self.viewport_override {
                Some(vp) => command_buffer.set_viewport(vp.clone()),
                None => command_buffer
                    .set_viewport(viewport(self.render_size.map(|number| number as f32))),
            };
            match &self.scissor_override {
                Some(scissor) => command_buffer.set_scissor(scissor.clone()),
                None => command_buffer.set_scissor(scissor(self.render_size, [0, 0])),
            };
        }

        // the context holds the command buffer borrow, so it is created after begin_rendering
        // and consumed by execute before end_rendering
        let graph_context =
            self.graph_context(*graph_node_id, command_buffer, inputs, outputs, report);
        node.node_type
            .execute(graph_context)
            .map_err(|error| GraphError::NodeFailed {
                node_name: node.name.clone(),
                source: Box::new(error),
            })?;

        if has_rendering {
            command_buffer.end_rendering();
        }
        Ok(())
    }

    /// Transitions an image to `new_layout`. `sort` validated the layouts and resource kinds
    /// and `render` created every resource before running jobs, so a failure here is a bug.
    fn add_resource_barrier(
        new_layout: ImageLayout,
        resource_id: ResourceId,
        layouts: &mut HashMap<ResourceId, ImageLayout>,
        resources: &HashMap<ResourceId, Arc<ResourceHandle>>,
        command_buffer: &mut ChaosCommandBuffer,
    ) {
        let old_layout = layouts
            .get(&resource_id)
            .copied()
            .unwrap_or(ImageLayout::Undefined);
        if new_layout == old_layout {
            return;
        }
        assert_ne!(
            new_layout,
            ImageLayout::Undefined,
            "can't transition to the undefined layout"
        );

        let resource = resources
            .get(&resource_id)
            .and_then(|handle| handle.resource.clone())
            .expect("render creates every resource before running jobs");
        let image = match resource.as_ref() {
            Resource::Texture(image) => image,
            other => panic!("image barrier on a resource that isn't a texture: {other:?}"),
        };
        Self::create_barrier(image.clone(), old_layout, new_layout, command_buffer);
        layouts.insert(resource_id, new_layout);
    }

    fn create_barrier(
        texture: Arc<ChaosTexture>,
        old_layout: ImageLayout,
        new_layout: ImageLayout,
        command_buffer: &mut ChaosCommandBuffer,
    ) {
        let (src_stages, _, src_writes) = layout_scope(old_layout)
            .unwrap_or_else(|| panic!("no barrier scope for image layout {old_layout:?}"));
        let (dst_stages, dst_reads, dst_writes) = layout_scope(new_layout)
            .unwrap_or_else(|| panic!("no barrier scope for image layout {new_layout:?}"));
        let image = texture.image();
        let mut barrier = ImageMemoryBarrier::image(image.clone());
        barrier.src_stages = src_stages;
        barrier.src_access = src_writes;
        barrier.dst_stages = dst_stages;
        barrier.dst_access = dst_reads | dst_writes;
        barrier.old_layout = old_layout;
        barrier.new_layout = new_layout;
        // covers every mip level, array layer and the aspects of the image's format
        barrier.subresource_range = image.subresource_range();

        // keep the image alive until the command buffer is submitted, otherwise it might get dropped before the barrier is executed
        command_buffer.keep(image.clone());

        command_buffer.pipeline_barrier(&DependencyInfo {
            image_memory_barriers: vec![barrier].into(),
            ..Default::default()
        });
    }

    /// Makes the writes of earlier nodes to a buffer visible to the node that consumes it.
    /// Buffers have no layout to track, so unlike images this is emitted for every use.
    fn add_buffer_barrier(
        usage: &BufferUsage,
        resource_id: ResourceId,
        resources: &HashMap<ResourceId, Arc<ResourceHandle>>,
        command_buffer: &mut ChaosCommandBuffer,
    ) -> Result<(), GraphError> {
        let resource = resources
            .get(&resource_id)
            .and_then(|handle| handle.resource.clone())
            .expect("render creates every resource before running jobs");
        let buffer = match resource.as_ref() {
            Resource::Buffer(buffer) => buffer,
            other => panic!("buffer barrier on a resource that isn't a buffer: {other:?}"),
        };
        // unlike the checks above this isn't a bug: the buffer's owner may not have uploaded data yet
        let subbuffer = buffer
            .buffer()
            .ok_or_else(|| RenderError::BufferNotUploaded {
                name: buffer.name.clone(),
            })?;

        let mut barrier = BufferMemoryBarrier::buffer(subbuffer.buffer().clone());
        barrier.src_stages = PipelineStages::ALL_COMMANDS;
        barrier.src_access = AccessFlags::MEMORY_WRITE;
        barrier.dst_stages = PipelineStages::VERTEX_SHADER
            | PipelineStages::FRAGMENT_SHADER
            | PipelineStages::COMPUTE_SHADER;
        barrier.dst_access = match usage {
            BufferUsage::StorageRead => AccessFlags::SHADER_STORAGE_READ,
            BufferUsage::StorageWrite => {
                AccessFlags::SHADER_STORAGE_READ | AccessFlags::SHADER_STORAGE_WRITE
            }
        };
        barrier.range = subbuffer.offset()..subbuffer.offset() + subbuffer.size();

        // keep the buffer alive until the command buffer is submitted, otherwise it might get dropped before the barrier is executed
        command_buffer.keep(subbuffer.buffer().clone());
        command_buffer.pipeline_barrier(&DependencyInfo {
            buffer_memory_barriers: vec![barrier].into(),
            ..Default::default()
        });

        Ok(())
    }

    /// Builds the parent and child lists that `compute_levels` schedules from.
    ///
    /// Edges come from two places:
    /// - every connection, source -> destination;
    /// - `resource_order_edges`: nodes that share a resource (in practice an import) are
    ///   ordered by the order they were added, so they can't race on it.
    fn create_hierarchy(
        &self,
    ) -> (
        HashMap<GraphNodeId, Vec<GraphNodeId>>,
        HashMap<GraphNodeId, Vec<GraphNodeId>>,
    ) {
        let mut parents: HashMap<GraphNodeId, Vec<GraphNodeId>> =
            self.nodes.iter().map(|(&id, _)| (id, Vec::new())).collect();
        let mut children: HashMap<GraphNodeId, Vec<GraphNodeId>> = parents.clone();

        let connection_edges = self
            .connections
            .iter()
            .map(|connection| (connection.source_id, connection.destination_id));
        let resource_edges = self
            .resource_order_edges()
            .into_iter()
            .map(|(parent, child, _)| (parent, child));

        for (parent, child) in connection_edges.chain(resource_edges) {
            parents.entry(child).or_default().push(parent);
            children.entry(parent).or_default().push(child);
        }

        (parents, children)
    }

    /// The ordering edges that come from sharing a resource, as `(parent, child, resource)`,
    /// sorted and without duplicates.
    ///
    /// - Writers: nodes that declare the resource themselves run in add order (node id), so
    ///   each writer gets an edge to the next one.
    /// - Readers: a node that reads the resource through a connection reads the version its
    ///   source wrote. The connection already puts it after that source; this adds an edge to
    ///   the next writer after the source, so the reader runs before it's overwritten.
    fn resource_order_edges(&self) -> Vec<(GraphNodeId, GraphNodeId, ResourceId)> {
        let mut writers: HashMap<ResourceId, Vec<GraphNodeId>> = HashMap::new();
        for ((node_id, _), resource_id) in &self.resource_lookup {
            writers.entry(*resource_id).or_default().push(*node_id);
        }

        let mut edges = vec![];
        for (resource_id, users) in writers.iter_mut() {
            // resource_lookup is a HashMap, so sort to get add order; a node can use the same
            // import under two names, so dedup to avoid a node -> itself edge
            users.sort();
            users.dedup();
            for pair in users.windows(2) {
                edges.push((pair[0], pair[1], *resource_id));
            }
        }

        for connection in &self.connections {
            // to_mermaid also runs on unvalidated graphs, so skip unknown sources
            let Some(resource_id) = self
                .resource_lookup
                .get(&(connection.source_id, connection.source_resource.clone()))
            else {
                continue;
            };
            let users = &writers[resource_id];
            let Ok(source_position) = users.binary_search(&connection.source_id) else {
                continue;
            };
            if let Some(&next_writer) = users.get(source_position + 1) {
                if next_writer != connection.destination_id {
                    edges.push((connection.destination_id, next_writer, *resource_id));
                }
            }
        }

        edges.sort_unstable();
        edges.dedup();
        edges
    }

    /// Groups nodes into dependency levels using Kahn's algorithm: every node in a level only
    /// depends on nodes from earlier levels, so nodes within a level can run in parallel.
    /// Nodes that are part of (or downstream of) a cycle never reach in-degree 0 and are
    /// left out of the result. Each level is sorted by node id to keep the output stable.
    fn compute_levels(
        &self,
        parents: HashMap<GraphNodeId, Vec<GraphNodeId>>,
        children: HashMap<GraphNodeId, Vec<GraphNodeId>>,
    ) -> Vec<Vec<GraphNodeId>> {
        let mut in_degree: HashMap<GraphNodeId, usize> = parents
            .iter()
            .map(|(nodeid, predecessors)| (*nodeid, predecessors.len()))
            .collect();

        let mut current: Vec<GraphNodeId> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&node_id, _)| node_id)
            .collect();
        let mut levels = Vec::new();

        while !current.is_empty() {
            current.sort_unstable();
            let mut next = Vec::new();
            for node_id in &current {
                for &successor in children.get(node_id).into_iter().flatten() {
                    if let Some(deg) = in_degree.get_mut(&successor) {
                        *deg -= 1;
                        if *deg == 0 {
                            next.push(successor);
                        }
                    }
                }
            }
            levels.push(current);
            current = next;
        }

        levels
    }

    /// Maps every connected input to the resource that feeds it. Only called after
    /// `validate`, which checked that every connection's source resource exists.
    fn resolve_inputs(&self) -> HashMap<(GraphNodeId, String), ResourceId> {
        self.connections
            .iter()
            .map(|connection| {
                let resource_id = *self
                    .resource_lookup
                    .get(&(connection.source_id, connection.source_resource.clone()))
                    .expect("validation ensured the source resource exists");
                (
                    (
                        connection.destination_id,
                        connection.destination_input.clone(),
                    ),
                    resource_id,
                )
            })
            .collect()
    }

    /// Builds a Mermaid flowchart representation of the graph: one box per node and one
    /// labeled edge per connection, showing which output resource feeds which input.
    /// Nodes are grouped into one subgraph per dependency level; nodes in the same level
    /// have no dependency on each other and can be parallelized. Nodes that can't be
    /// scheduled because of a cycle are shown in a separate "Unscheduled" subgraph.
    /// Each node is coloured by the queue (graphics/compute/transfer) it is meant to run on.
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("```mermaid\nflowchart TD\n");

        let (parents, children) = self.create_hierarchy();
        let levels = self.compute_levels(parents, children);

        let scheduled: HashSet<GraphNodeId> = levels.iter().flatten().copied().collect();
        let mut unscheduled: Vec<GraphNodeId> = self
            .nodes
            .keys()
            .filter(|node_id| !scheduled.contains(node_id))
            .copied()
            .collect();
        unscheduled.sort_unstable();

        // Declare every node up front so isolated (unconnected) nodes still show up.
        for (index, level) in levels.iter().enumerate() {
            self.push_mermaid_subgraph(
                &mut mermaid,
                &format!("level_{}", index),
                &format!("Level {}", index),
                level,
            );
        }
        if !unscheduled.is_empty() {
            self.push_mermaid_subgraph(
                &mut mermaid,
                "unscheduled",
                "Unscheduled (circular connection)",
                &unscheduled,
            );
        }

        for connection in &self.connections {
            let resource_name = self.nodes[&connection.source_id]
                .outputs
                .contains(&connection.source_resource)
                .then(|| connection.source_resource.clone())
                .unwrap_or_else(|| format!("resource_{}", connection.source_resource));

            mermaid.push_str(&format!(
                "    node_{} -->|\"{} -> {}\"| node_{}\n",
                connection.source_id,
                resource_name,
                connection.destination_input,
                connection.destination_id
            ));
        }

        // ordering that comes from sharing a resource rather than from a connection; drawn
        // dotted, and skipped where a connection already shows the same edge
        let connected: HashSet<(GraphNodeId, GraphNodeId)> = self
            .connections
            .iter()
            .map(|connection| (connection.source_id, connection.destination_id))
            .collect();
        for (parent, child, resource_id) in self.resource_order_edges() {
            if connected.contains(&(parent, child)) {
                continue;
            }
            let resource_name = self
                .imported_resources
                .iter()
                .find(|(_, id)| **id == resource_id)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| format!("resource_{}", resource_id));
            mermaid.push_str(&format!(
                "    node_{} -.->|\"{}\"| node_{}\n",
                parent, resource_name, child
            ));
        }

        if !self.nodes.is_empty() {
            for queue in QueueKind::ALL {
                mermaid.push_str(&format!(
                    "    classDef {} {}\n",
                    queue.name(),
                    queue.mermaid_style()
                ));
            }
        }

        mermaid.push_str("```");
        return mermaid;
    }

    fn push_mermaid_subgraph(
        &self,
        mermaid: &mut String,
        id: &str,
        label: &str,
        node_ids: &[GraphNodeId],
    ) {
        mermaid.push_str(&format!("    subgraph {}[\"{}\"]\n", id, label));
        for node_id in node_ids {
            let node = &self.nodes[node_id];
            mermaid.push_str(&format!(
                "        node_{}[\"{}\"]:::{}\n",
                node_id,
                node.name,
                node.queue.name()
            ));
        }
        mermaid.push_str("    end\n");
    }
}

/// Returns what an input expected if `description` can't satisfy it.
fn kind_mismatch(kind: &NodeInputKind, description: &ResourceDescription) -> Option<&'static str> {
    match (kind, description) {
        #[cfg(test)]
        (NodeInputKind::TestKind, _) => None,
        #[cfg(test)]
        (_, ResourceDescription::TestTextureResource | ResourceDescription::TestBufferResource) => {
            None
        }
        (NodeInputKind::Texture(_), ResourceDescription::Texture(_)) => None,
        (NodeInputKind::Texture(_), ResourceDescription::Imported { .. }) => None,
        // there is no buffer resource description yet, so nothing can feed a buffer input
        (NodeInputKind::Buffer(_), _) => Some("buffer"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::command_buffer::ChaosClearValue::Float;
    use crate::rendering::command_buffer::{ChaosClearValue, ChaosRenderAttachmentLoadOp};
    use crate::rendering::graph::{
        node::GraphNodeType,
        resource::{ImportedResourceSetting, ResourceDescription, ResourceUsage},
    };
    use std::assert_eq;

    pub struct TestNode {}

    impl GraphNodeType for TestNode {
        fn execute(&self, _ctx: GraphContext) -> Result<(), GraphError> {
            return Ok(());
        }
    }

    fn test_node(inputs: Vec<String>, outputs: Vec<String>) -> GraphNode {
        test_named_node("Test Node".to_string(), inputs, outputs)
    }

    fn test_named_node(name: String, inputs: Vec<String>, outputs: Vec<String>) -> GraphNode {
        let mut node = GraphNode::new(name, TestNode {});
        for input in inputs {
            node = node.with_input(input, NodeInputKind::TestKind);
        }
        for output in outputs {
            node = node.with_resource(
                output,
                ResourceDescription::TestTextureResource,
                ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                true,
            );
        }
        node
    }

    #[test]
    fn test_to_mermaid_empty_graph() {
        let graph = Graph::new();
        let mermaid = graph.to_mermaid();
        assert_eq!(mermaid, "```mermaid\nflowchart TD\n```");
    }

    #[test]
    fn test_to_mermaid_non_empty_graph() {
        let mut graph = Graph::new();
        let node_id = graph.add_node(test_node(vec![], vec![])).unwrap();
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains(format!("node_{}", node_id).as_str()));
        assert!(mermaid.contains("Test Node"));
    }

    #[test]
    fn test_to_mermaid_graph_with_connection() {
        let mut graph = Graph::new();
        let source_id = graph
            .add_node(test_named_node(
                "Source Node".to_string(),
                vec![],
                vec!["source_output".to_string()],
            ))
            .unwrap();
        let destination_id = graph
            .add_node(test_named_node(
                "Destination Node".to_string(),
                vec!["input".to_string()],
                vec![],
            ))
            .unwrap();

        let result = graph.connect(
            source_id,
            "source_output".to_string(),
            destination_id,
            "input".to_string(),
        );

        assert!(result.is_ok(),);
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains(format!("node_{}", source_id).as_str()));
        assert!(mermaid.contains(format!("node_{}", destination_id).as_str()));
        assert!(mermaid.contains("Source Node"));
        assert!(mermaid.contains("Destination Node"));
    }

    #[test]
    fn test_to_mermaid_groups_parallel_nodes_into_levels() {
        let mut graph = Graph::new();
        let root = graph
            .add_node(test_named_node(
                "Root".to_string(),
                vec![],
                vec!["out".to_string()],
            ))
            .unwrap();
        let left = graph
            .add_node(test_named_node(
                "Left".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        let right = graph
            .add_node(test_named_node(
                "Right".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        let join = graph
            .add_node(test_named_node(
                "Join".to_string(),
                vec!["a".to_string(), "b".to_string()],
                vec![],
            ))
            .unwrap();
        graph
            .connect(root, "out".to_string(), left, "in".to_string())
            .unwrap();
        graph
            .connect(root, "out".to_string(), right, "in".to_string())
            .unwrap();
        graph
            .connect(left, "out".to_string(), join, "a".to_string())
            .unwrap();
        graph
            .connect(right, "out".to_string(), join, "b".to_string())
            .unwrap();

        let (parents, children) = graph.create_hierarchy();

        assert_eq!(
            graph.compute_levels(parents, children),
            vec![vec![root], vec![left, right], vec![join]]
        );

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("subgraph level_0[\"Level 0\"]"));
        assert!(mermaid.contains("subgraph level_1[\"Level 1\"]"));
        assert!(mermaid.contains("subgraph level_2[\"Level 2\"]"));
    }

    #[test]
    fn test_to_mermaid_colours_nodes_by_queue() {
        let mut graph = Graph::new();
        let gfx = graph
            .add_node(test_named_node("Gfx".to_string(), vec![], vec![]))
            .unwrap();
        let comp = graph
            .add_node(
                test_named_node("Comp".to_string(), vec![], vec![]).with_queue(QueueKind::Compute),
            )
            .unwrap();

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains(&format!("node_{}[\"Gfx\"]:::graphics", gfx)));
        assert!(mermaid.contains(&format!("node_{}[\"Comp\"]:::compute", comp)));
        assert!(mermaid.contains("classDef graphics"));
        assert!(mermaid.contains("classDef compute"));
        assert!(mermaid.contains("classDef transfer"));
    }

    #[test]
    fn test_to_mermaid_isolated_nodes_share_first_level() {
        let mut graph = Graph::new();
        let a = graph.add_node(test_node(vec![], vec![])).unwrap();
        let b = graph.add_node(test_node(vec![], vec![])).unwrap();
        let (parents, children) = graph.create_hierarchy();
        assert_eq!(graph.compute_levels(parents, children), vec![vec![a, b]]);
    }

    #[test]
    fn test_to_mermaid_marks_cycle_as_unscheduled() {
        let mut graph = Graph::new();
        let free = graph
            .add_node(test_named_node("Free".to_string(), vec![], vec![]))
            .unwrap();
        let a = graph
            .add_node(test_named_node(
                "A".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        let b = graph
            .add_node(test_named_node(
                "B".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        graph
            .connect(a, "out".to_string(), b, "in".to_string())
            .unwrap();
        graph
            .connect(b, "out".to_string(), a, "in".to_string())
            .unwrap();

        let (parents, children) = graph.create_hierarchy();
        assert_eq!(graph.compute_levels(parents, children), vec![vec![free]]);
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("subgraph unscheduled"));
    }

    fn executed_node_ids_per_level(graph: &Graph) -> Vec<Vec<GraphNodeId>> {
        graph
            .jobs
            .iter()
            .map(|level| {
                level
                    .iter()
                    .filter_map(|job| match job {
                        Job::NodeExecutionJob { graph_node_id, .. } => Some(*graph_node_id),
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_sort_schedules_isolated_nodes_in_id_order() {
        let mut graph = Graph::new();
        let ids: Vec<GraphNodeId> = (0..5)
            .map(|_| graph.add_node(test_node(vec![], vec![])).unwrap())
            .collect();

        assert!(graph.sort().is_ok());
        assert_eq!(executed_node_ids_per_level(&graph), vec![ids]);
    }

    #[test]
    fn test_sort_orders_diamond_by_levels() {
        let mut graph = Graph::new();
        let root = graph
            .add_node(test_named_node(
                "Root".to_string(),
                vec![],
                vec!["out".to_string()],
            ))
            .unwrap();
        let left = graph
            .add_node(test_named_node(
                "Left".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        let right = graph
            .add_node(test_named_node(
                "Right".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        let join = graph
            .add_node(test_named_node(
                "Join".to_string(),
                vec!["a".to_string(), "b".to_string()],
                vec![],
            ))
            .unwrap();
        graph
            .connect(root, "out".to_string(), left, "in".to_string())
            .unwrap();
        graph
            .connect(root, "out".to_string(), right, "in".to_string())
            .unwrap();
        graph
            .connect(left, "out".to_string(), join, "a".to_string())
            .unwrap();
        graph
            .connect(right, "out".to_string(), join, "b".to_string())
            .unwrap();

        assert!(graph.sort().is_ok());
        assert_eq!(
            executed_node_ids_per_level(&graph),
            vec![vec![root], vec![left, right], vec![join]]
        );
    }

    fn execution_bindings(
        graph: &Graph,
        node_id: GraphNodeId,
    ) -> (Vec<(String, ResourceId)>, Vec<(String, ResourceId)>) {
        graph
            .jobs
            .iter()
            .flatten()
            .find_map(|job| match job {
                Job::NodeExecutionJob {
                    graph_node_id,
                    inputs,
                    outputs,
                } if *graph_node_id == node_id => Some((inputs.clone(), outputs.clone())),
                _ => None,
            })
            .expect("node should have an execution job")
    }

    #[test]
    fn test_sort_binds_inputs_by_destination_name_and_outputs_by_own_name() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(test_named_node(
                "Source".to_string(),
                vec![],
                vec!["source_out".to_string()],
            ))
            .unwrap();
        let sink = graph
            .add_node(test_named_node(
                "Sink".to_string(),
                vec!["albedo".to_string()],
                vec!["sink_out".to_string()],
            ))
            .unwrap();
        graph
            .connect(source, "source_out".to_string(), sink, "albedo".to_string())
            .unwrap();

        assert!(graph.sort().is_ok());

        let source_out = graph.resource_lookup[&(source, "source_out".to_string())];
        let sink_out = graph.resource_lookup[&(sink, "sink_out".to_string())];
        let (source_inputs, source_outputs) = execution_bindings(&graph, source);
        let (sink_inputs, sink_outputs) = execution_bindings(&graph, sink);

        assert!(source_inputs.is_empty());
        assert_eq!(source_outputs, vec![("source_out".to_string(), source_out)]);
        // the sink asks for "albedo", not for the name of the upstream output
        assert_eq!(sink_inputs, vec![("albedo".to_string(), source_out)]);
        assert_eq!(sink_outputs, vec![("sink_out".to_string(), sink_out)]);
    }

    #[test]
    fn test_sort_binds_internal_resources_but_does_not_expose_them() {
        let mut graph = Graph::new();
        let node = graph
            .add_node(
                GraphNode::new("Node".to_string(), TestNode {})
                    .with_resource(
                        "exposed".to_string(),
                        ResourceDescription::TestTextureResource,
                        ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                        true,
                    )
                    .with_resource(
                        "scratch".to_string(),
                        ResourceDescription::TestTextureResource,
                        ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                        false,
                    ),
            )
            .unwrap();
        let consumer = graph
            .add_node(test_named_node(
                "Consumer".to_string(),
                vec!["in".to_string()],
                vec![],
            ))
            .unwrap();

        assert!(
            graph
                .connect(node, "scratch".to_string(), consumer, "in".to_string())
                .is_err(),
            "internal resources can't be connected to other nodes"
        );
        graph
            .connect(node, "exposed".to_string(), consumer, "in".to_string())
            .unwrap();
        assert!(graph.sort().is_ok());

        let exposed = graph.resource_lookup[&(node, "exposed".to_string())];
        let scratch = graph.resource_lookup[&(node, "scratch".to_string())];
        let (_, outputs) = execution_bindings(&graph, node);
        assert_eq!(
            outputs,
            vec![
                ("exposed".to_string(), exposed),
                ("scratch".to_string(), scratch)
            ]
        );
    }

    #[test]
    fn test_sort_emits_buffer_barrier_for_buffer_inputs() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(test_named_node(
                "Source".to_string(),
                vec![],
                vec!["out".to_string()],
            ))
            .unwrap();
        let sink = graph
            .add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
                "data".to_string(),
                NodeInputKind::Buffer(BufferUsage::StorageRead),
            ))
            .unwrap();
        graph
            .connect(source, "out".to_string(), sink, "data".to_string())
            .unwrap();

        assert!(graph.sort().is_ok());

        let out = graph.resource_lookup[&(source, "out".to_string())];
        assert!(graph.jobs.iter().flatten().any(|job| matches!(
            job,
            Job::BufferBarrierJob { resource_id, usage: BufferUsage::StorageRead } if *resource_id == out
        )));
        // the only image barrier on `out` is the producer's own attachment usage, the buffer
        // input must not add another one
        let image_barriers = graph
            .jobs
            .iter()
            .flatten()
            .filter(|job| matches!(job, Job::BarrierJob { resource_id, .. } if *resource_id == out))
            .count();
        assert_eq!(image_barriers, 1);
    }

    #[test]
    fn test_node_resources_get_by_name() {
        let handle = Arc::new(ResourceHandle::new(
            &ResourceDescription::TestTextureResource,
            "hdr",
        ));
        let resources = NodeResources::new(vec![("color".to_string(), handle.clone())]);

        assert!(Arc::ptr_eq(resources.get("color").unwrap(), &handle));
        assert!(matches!(
            resources.get("hdr").unwrap_err(),
            GraphError::NodeResourceNotFound { resource_name } if resource_name == "hdr"
        ));
    }

    #[test]
    fn test_circular_detection() {
        let mut graph = Graph::new();
        let node_a = graph
            .add_node(test_named_node(
                "Node A".to_string(),
                vec!["input".to_string()],
                vec!["output".to_string()],
            ))
            .unwrap();
        let node_b = graph
            .add_node(test_named_node(
                "Node B".to_string(),
                vec!["input".to_string()],
                vec!["output".to_string()],
            ))
            .unwrap();
        let node_c = graph
            .add_node(test_named_node(
                "Node C".to_string(),
                vec!["input".to_string()],
                vec!["output".to_string()],
            ))
            .unwrap();

        assert!(
            graph
                .connect(node_a, "output".to_string(), node_b, "input".to_string())
                .is_ok()
        );
        assert!(
            graph
                .connect(node_b, "output".to_string(), node_c, "input".to_string())
                .is_ok()
        );
        assert!(
            graph
                .connect(node_c, "output".to_string(), node_a, "input".to_string())
                .is_ok(),
        );
        let result = graph.sort();
        assert!(
            result.is_err(),
            "Expected sort to fail due to circular connection"
        );
        match result {
            Err(GraphError::Invalid(problems)) => match problems.as_slice() {
                [GraphBuildError::CircularConnection(nodes)] => {
                    // check if the nodes involved in the circular connection are as expected
                    assert!(nodes.contains(&"Node A".to_string()));
                    assert!(nodes.contains(&"Node B".to_string()));
                    assert!(nodes.contains(&"Node C".to_string()))
                }
                other => panic!("Expected only a circular connection error, got {other:?}"),
            },
            _ => panic!("Expected circular connection error"),
        }
    }

    #[test]
    fn test_sort_reports_every_problem_at_once() {
        let mut graph = Graph::new();
        // two unconnected inputs on one node, plus a cycle elsewhere
        graph
            .add_node(test_named_node(
                "Lonely".to_string(),
                vec!["a".to_string(), "b".to_string()],
                vec![],
            ))
            .unwrap();
        let x = graph
            .add_node(test_named_node(
                "X".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        let y = graph
            .add_node(test_named_node(
                "Y".to_string(),
                vec!["in".to_string()],
                vec!["out".to_string()],
            ))
            .unwrap();
        graph
            .connect(x, "out".to_string(), y, "in".to_string())
            .unwrap();
        graph
            .connect(y, "out".to_string(), x, "in".to_string())
            .unwrap();

        let Err(GraphError::Invalid(problems)) = graph.sort() else {
            panic!("expected sort to fail");
        };
        assert_eq!(
            problems,
            vec![
                GraphBuildError::InputNotConnected {
                    node_name: "Lonely".to_string(),
                    input_name: "a".to_string(),
                },
                GraphBuildError::InputNotConnected {
                    node_name: "Lonely".to_string(),
                    input_name: "b".to_string(),
                },
                GraphBuildError::CircularConnection(vec!["X".to_string(), "Y".to_string()]),
            ]
        );
    }

    fn texture_description() -> ResourceDescription {
        ResourceDescription::Texture(crate::rendering::graph::resource::TextureDescription {
            format: vulkano::format::Format::R8G8B8A8_UNORM,
            dimensions: [100, 100, 1],
            mip_levels: 1,
            array_size: 1,
            dimension_calculations: [
                TextureSizeType::PercentageOfRenderWidth,
                TextureSizeType::PercentageOfRenderHeight,
                TextureSizeType::Fixed,
            ],
            usage: vulkano::image::ImageUsage::COLOR_ATTACHMENT,
        })
    }

    #[test]
    fn test_sort_rejects_texture_connected_to_buffer_input() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(
                GraphNode::new("Source".to_string(), TestNode {}).with_resource(
                    "color".to_string(),
                    texture_description(),
                    ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                    true,
                ),
            )
            .unwrap();
        let sink = graph
            .add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
                "data".to_string(),
                NodeInputKind::Buffer(BufferUsage::StorageRead),
            ))
            .unwrap();
        graph
            .connect(source, "color".to_string(), sink, "data".to_string())
            .unwrap();

        let Err(GraphError::Invalid(problems)) = graph.sort() else {
            panic!("expected sort to fail");
        };
        assert_eq!(
            problems,
            vec![GraphBuildError::InputKindMismatch {
                node_name: "Sink".to_string(),
                input_name: "data".to_string(),
                expected: "buffer",
                source_node_name: "Source".to_string(),
                source_resource: "color".to_string(),
            }]
        );
    }

    #[test]
    fn test_sort_rejects_unsupported_input_layout() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(
                GraphNode::new("Source".to_string(), TestNode {}).with_resource(
                    "color".to_string(),
                    texture_description(),
                    ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                    true,
                ),
            )
            .unwrap();
        let sink = graph
            .add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
                "color".to_string(),
                NodeInputKind::Texture(ImageLayout::Undefined),
            ))
            .unwrap();
        graph
            .connect(source, "color".to_string(), sink, "color".to_string())
            .unwrap();

        let Err(GraphError::Invalid(problems)) = graph.sort() else {
            panic!("expected sort to fail");
        };
        assert_eq!(
            problems,
            vec![GraphBuildError::InvalidInputLayout {
                node_name: "Sink".to_string(),
                input_name: "color".to_string(),
                layout: ImageLayout::Undefined,
            }]
        );
    }

    #[test]
    fn test_connecting_to_same_node_returns_err() {
        let mut graph = Graph::new();
        let node_id = graph
            .add_node(test_node(
                vec!["input".to_string()],
                vec!["output".to_string()],
            ))
            .unwrap();
        let result = graph.connect(node_id, "output".to_string(), node_id, "input".to_string());
        assert!(
            result.is_err(),
            "Expected connecting to the same node to return an error"
        );
    }

    // ---- imported resources ----

    fn imported(
        resource_name: &str,
        initial: ImageLayout,
        end: ImageLayout,
    ) -> ResourceDescription {
        ResourceDescription::Imported {
            resource_name: resource_name.to_string(),
            setting: ImportedResourceSetting {
                initial_layout: initial,
                end_layout: end,
            },
        }
    }

    fn swapchain() -> ResourceDescription {
        imported("swapchain", ImageLayout::Undefined, ImageLayout::PresentSrc)
    }

    fn node_writing(name: &str, resource: &str, description: ResourceDescription) -> GraphNode {
        node_using(
            name,
            resource,
            description,
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
        )
    }

    fn node_using(
        name: &str,
        resource: &str,
        description: ResourceDescription,
        usage: ResourceUsage,
    ) -> GraphNode {
        GraphNode::new(name.to_string(), TestNode {}).with_resource(
            resource.to_string(),
            description,
            usage,
            true,
        )
    }

    /// Index of every job touching `resource_id`, flattened across levels, in execution order.
    fn jobs_touching(graph: &Graph, resource_id: ResourceId) -> Vec<(usize, Job)> {
        graph
            .jobs
            .iter()
            .flatten()
            .cloned()
            .enumerate()
            .filter(|(_, job)| match job {
                Job::BarrierJob {
                    resource_id: id, ..
                } => *id == resource_id,
                Job::BufferBarrierJob {
                    resource_id: id, ..
                } => *id == resource_id,
                Job::NodeExecutionJob {
                    inputs, outputs, ..
                } => inputs
                    .iter()
                    .chain(outputs.iter())
                    .any(|(_, id)| *id == resource_id),
            })
            .collect()
    }

    #[test]
    fn test_imported_resource_is_shared_between_nodes_by_name() {
        let mut graph = Graph::new();
        let a = graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();
        // a different node-local name, but the same imported name
        let b = graph
            .add_node(node_writing("B", "target", swapchain()))
            .unwrap();

        let a_id = graph.resource_lookup[&(a, "color".to_string())];
        let b_id = graph.resource_lookup[&(b, "target".to_string())];
        assert_eq!(a_id, b_id);
        assert_eq!(graph.resources.len(), 1);
        assert_eq!(graph.imported_resources["swapchain"], a_id);
    }

    #[test]
    fn test_differently_named_imports_get_separate_resources() {
        let mut graph = Graph::new();
        let a = graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();
        let b = graph
            .add_node(node_writing(
                "B",
                "color",
                imported("other", ImageLayout::Undefined, ImageLayout::PresentSrc),
            ))
            .unwrap();

        assert_ne!(
            graph.resource_lookup[&(a, "color".to_string())],
            graph.resource_lookup[&(b, "color".to_string())]
        );
        assert_eq!(graph.imported_resources.len(), 2);
    }

    #[test]
    fn test_imported_resource_does_not_consume_extra_ids() {
        let mut graph = Graph::new();
        graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();
        graph
            .add_node(node_writing("B", "color", swapchain()))
            .unwrap();
        let c = graph
            .add_node(test_named_node(
                "C".to_string(),
                vec![],
                vec!["out".to_string()],
            ))
            .unwrap();

        // the shared import took one id, so the next regular resource gets the next one
        let swapchain_id = graph.imported_resources["swapchain"];
        assert_eq!(
            graph.resource_lookup[&(c, "out".to_string())],
            swapchain_id + 1
        );
    }

    #[test]
    fn test_imported_resource_mismatching_settings_returns_err() {
        let mut graph = Graph::new();
        graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();

        let result = graph.add_node(node_writing(
            "B",
            "color",
            imported(
                "swapchain",
                ImageLayout::Undefined,
                ImageLayout::ShaderReadOnlyOptimal,
            ),
        ));
        assert!(matches!(
            result,
            Err(GraphBuildError::ImportedResourceSettingMismatch { resource_name })
                if resource_name == "swapchain"
        ));

        let result = graph.add_node(node_writing(
            "C",
            "color",
            imported(
                "swapchain",
                ImageLayout::ColorAttachmentOptimal,
                ImageLayout::PresentSrc,
            ),
        ));
        assert!(matches!(
            result,
            Err(GraphBuildError::ImportedResourceSettingMismatch { .. })
        ));
    }

    #[test]
    fn test_imported_resource_can_feed_texture_input() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(node_writing("Source", "color", swapchain()))
            .unwrap();
        let sink = graph
            .add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
                "color".to_string(),
                NodeInputKind::Texture(ImageLayout::ShaderReadOnlyOptimal),
            ))
            .unwrap();
        graph
            .connect(source, "color".to_string(), sink, "color".to_string())
            .unwrap();

        assert!(graph.sort().is_ok());
    }

    #[test]
    fn test_imported_resource_ends_in_end_layout_after_last_use() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(node_writing("Source", "color", swapchain()))
            .unwrap();
        let sink = graph
            .add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
                "color".to_string(),
                NodeInputKind::Texture(ImageLayout::ShaderReadOnlyOptimal),
            ))
            .unwrap();
        graph
            .connect(source, "color".to_string(), sink, "color".to_string())
            .unwrap();
        assert!(graph.sort().is_ok());

        let swapchain_id = graph.imported_resources["swapchain"];
        let jobs = jobs_touching(&graph, swapchain_id);

        let last_execution = jobs
            .iter()
            .filter(|(_, job)| matches!(job, Job::NodeExecutionJob { .. }))
            .map(|(index, _)| *index)
            .max()
            .expect("the swapchain is used by a node");
        let (last_job_index, last_job) = jobs.last().unwrap();

        assert!(
            *last_job_index > last_execution,
            "the end-layout barrier must come after the last node using the resource, jobs: {jobs:?}"
        );
        assert!(
            matches!(
                last_job,
                Job::BarrierJob {
                    image_layout: ImageLayout::PresentSrc,
                    ..
                }
            ),
            "the last job on the resource must transition it to its end layout, got {last_job:?}"
        );
    }

    #[test]
    fn test_imported_resource_gets_exactly_one_end_barrier() {
        let mut graph = Graph::new();
        graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();
        assert!(graph.sort().is_ok());

        let swapchain_id = graph.imported_resources["swapchain"];
        let end_barriers = jobs_touching(&graph, swapchain_id)
            .iter()
            .filter(|(_, job)| {
                matches!(
                    job,
                    Job::BarrierJob {
                        image_layout: ImageLayout::PresentSrc,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(end_barriers, 1);
    }

    #[test]
    fn test_regular_resources_get_no_end_barrier() {
        let mut graph = Graph::new();
        let source = graph
            .add_node(test_named_node(
                "Source".to_string(),
                vec![],
                vec!["out".to_string()],
            ))
            .unwrap();
        assert!(graph.sort().is_ok());

        let out = graph.resource_lookup[&(source, "out".to_string())];
        let barriers = jobs_touching(&graph, out)
            .iter()
            .filter(|(_, job)| matches!(job, Job::BarrierJob { .. }))
            .count();
        // only the producer's own attachment barrier
        assert_eq!(barriers, 1);
    }

    #[test]
    fn test_writers_of_the_same_import_are_ordered_by_add_order() {
        let mut graph = Graph::new();
        graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();
        graph
            .add_node(node_writing("B", "color", swapchain()))
            .unwrap();
        assert!(graph.sort().is_ok());

        // writers of the same imported resource are put in separate levels, in the order they
        // were added, so their writes don't race
        let swapchain_id = graph.imported_resources["swapchain"];
        let writers_per_level: Vec<usize> = graph
            .jobs
            .iter()
            .map(|level| {
                level
                    .iter()
                    .filter(|job| match job {
                        Job::NodeExecutionJob { outputs, .. } => {
                            outputs.iter().any(|(_, id)| *id == swapchain_id)
                        }
                        _ => false,
                    })
                    .count()
            })
            .collect();
        assert_eq!(writers_per_level, vec![1, 1]);
    }

    #[test]
    fn test_clear_then_load_on_same_resource_get_ordered_correctly() {
        // Clear then Load, no connector. A (Clear) is added before B (Load), both on swapchain().
        // Expect no LoadBeforeWrite: add order puts A in an earlier level than B, so the clear
        // always happens first.
        let node_a = node_using(
            "A",
            "color",
            swapchain(),
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Clear(Float([
                0.0, 0.0, 0.0, 1.0,
            ]))),
        );

        let node_b = node_using(
            "B",
            "color",
            swapchain(),
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Load),
        );
        let mut graph = Graph::new();
        graph.add_node(node_a).unwrap();
        graph.add_node(node_b).unwrap();
        assert!(graph.sort().is_ok());
    }

    #[test]
    fn test_load_then_clear_on_same_resource_get_ordered_correctly() {
        // Load then Clear, no connector. B (Load) is added before A (Clear), so add order runs
        // B first and it loads a swapchain nothing has written yet.
        let node_a = node_using(
            "A",
            "color",
            swapchain(),
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Clear(Float([
                0.0, 0.0, 0.0, 1.0,
            ]))),
        );

        let node_b = node_using(
            "B",
            "color",
            swapchain(),
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Load),
        );
        let mut graph = Graph::new();
        graph.add_node(node_b).unwrap();
        graph.add_node(node_a).unwrap();
        let problems = sort_problems(&mut graph);
        assert_eq!(
            load_before_write(&problems),
            vec![("B", "color")],
            "got {problems:?}"
        );
    }

    #[test]
    fn test_end_barriers_in_one_level_are_ordered_by_resource_id() {
        // several imports ending in the same level; with HashMap iteration a lucky order
        // for all of them is unlikely, so this catches a regression to unordered emission
        let names = ["e", "b", "d", "a", "c", "f"];
        let mut graph = Graph::new();
        for name in names {
            graph
                .add_node(node_writing(
                    &format!("Writer {name}"),
                    "color",
                    imported(name, ImageLayout::Undefined, ImageLayout::PresentSrc),
                ))
                .unwrap();
        }
        assert!(graph.sort().is_ok());
        assert_eq!(graph.jobs.len(), 1, "independent writers share one level");

        let mut expected: Vec<ResourceId> = graph.imported_resources.values().copied().collect();
        expected.sort();

        // the end barriers are the last jobs of the level, after every node has run
        let level = &graph.jobs[0];
        let tail = &level[level.len() - expected.len()..];
        let end_barrier_ids: Vec<ResourceId> = tail
            .iter()
            .map(|job| match job {
                Job::BarrierJob {
                    resource_id,
                    image_layout: ImageLayout::PresentSrc,
                } => *resource_id,
                other => panic!("expected an end-layout barrier, got {other:?}"),
            })
            .collect();

        assert_eq!(end_barrier_ids, expected);
    }

    fn color_clear(value: ChaosClearValue) -> ResourceUsage {
        ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Clear(value))
    }

    fn depth_clear(value: ChaosClearValue) -> ResourceUsage {
        ResourceUsage::DepthAttachment(ChaosRenderAttachmentLoadOp::Clear(value))
    }

    /// Sorts a graph with a single node using `usage` and returns the problems `sort` found.
    fn problems_for_usage(usage: ResourceUsage) -> Vec<GraphBuildError> {
        let mut graph = Graph::new();
        graph
            .add_node(node_using(
                "Node",
                "target",
                ResourceDescription::TestTextureResource,
                usage,
            ))
            .unwrap();
        match graph.sort() {
            Ok(()) => vec![],
            Err(GraphError::Invalid(problems)) => problems,
            Err(other) => panic!("expected a build error, got {other:?}"),
        }
    }

    #[test]
    fn test_color_attachment_with_depth_clear_value_is_rejected() {
        let problems = problems_for_usage(color_clear(ChaosClearValue::Depth(1.0)));
        assert!(
            matches!(
                problems.as_slice(),
                [GraphBuildError::InvalidClearValue { node_name, resource_name, .. }]
                    if node_name == "Node" && resource_name == "target"
            ),
            "got {problems:?}"
        );
    }

    #[test]
    fn test_color_attachment_with_stencil_clear_values_is_rejected() {
        for value in [
            ChaosClearValue::Stencil(0),
            ChaosClearValue::DepthStencil(1.0, 0),
        ] {
            let problems = problems_for_usage(color_clear(value.clone()));
            assert!(
                matches!(
                    problems.as_slice(),
                    [GraphBuildError::InvalidClearValue { .. }]
                ),
                "{value:?} on a color attachment, got {problems:?}"
            );
        }
    }

    #[test]
    fn test_depth_attachment_with_color_clear_values_is_rejected() {
        for value in [
            ChaosClearValue::Float([0.0, 0.0, 0.0, 1.0]),
            ChaosClearValue::Int([0; 4]),
            ChaosClearValue::Uint([0; 4]),
            ChaosClearValue::Stencil(0),
        ] {
            let problems = problems_for_usage(depth_clear(value.clone()));
            assert!(
                matches!(
                    problems.as_slice(),
                    [GraphBuildError::InvalidClearValue { node_name, resource_name, .. }]
                        if node_name == "Node" && resource_name == "target"
                ),
                "{value:?} on a depth attachment, got {problems:?}"
            );
        }
    }

    #[test]
    fn test_matching_clear_values_are_accepted() {
        let usages = [
            color_clear(ChaosClearValue::Float([0.0, 0.0, 0.0, 1.0])),
            color_clear(ChaosClearValue::Int([0; 4])),
            color_clear(ChaosClearValue::Uint([0; 4])),
            depth_clear(ChaosClearValue::Depth(1.0)),
            depth_clear(ChaosClearValue::DepthStencil(1.0, 0)),
        ];
        for usage in usages {
            let problems = problems_for_usage(usage.clone());
            assert!(problems.is_empty(), "{usage:?} got {problems:?}");
        }
    }

    #[test]
    fn test_usages_without_clear_values_are_accepted() {
        // Load is checked by the load-before-write rule, not here, so a single DontCare/Load
        // node only has to pass the clear-value check
        let usages = [
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            ResourceUsage::DepthAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            ResourceUsage::StorageWrite,
            ResourceUsage::TransferDst,
        ];
        for usage in usages {
            let problems = problems_for_usage(usage.clone());
            assert!(
                !problems
                    .iter()
                    .any(|p| matches!(p, GraphBuildError::InvalidClearValue { .. })),
                "{usage:?} got {problems:?}"
            );
        }
    }

    #[test]
    fn test_every_invalid_clear_value_is_reported() {
        let mut graph = Graph::new();
        graph
            .add_node(
                GraphNode::new("Node".to_string(), TestNode {})
                    .with_resource(
                        "color".to_string(),
                        ResourceDescription::TestTextureResource,
                        color_clear(ChaosClearValue::Depth(1.0)),
                        true,
                    )
                    .with_resource(
                        "depth".to_string(),
                        ResourceDescription::TestTextureResource,
                        depth_clear(ChaosClearValue::Float([0.0; 4])),
                        true,
                    ),
            )
            .unwrap();

        let Err(GraphError::Invalid(problems)) = graph.sort() else {
            panic!("expected the graph to be invalid");
        };
        let names: Vec<&str> = problems
            .iter()
            .filter_map(|p| match p {
                GraphBuildError::InvalidClearValue { resource_name, .. } => {
                    Some(resource_name.as_str())
                }
                _ => None,
            })
            .collect();
        // sorted by resource name, so the order is stable
        assert_eq!(names, vec!["color", "depth"]);
    }

    fn color_load() -> ResourceUsage {
        ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Load)
    }

    fn depth_load() -> ResourceUsage {
        ResourceUsage::DepthAttachment(ChaosRenderAttachmentLoadOp::Load)
    }

    /// Names (node, resource) of every `LoadBeforeWrite` in `problems`.
    fn load_before_write(problems: &[GraphBuildError]) -> Vec<(&str, &str)> {
        problems
            .iter()
            .filter_map(|p| match p {
                GraphBuildError::LoadBeforeWrite {
                    node_name,
                    resource_name,
                } => Some((node_name.as_str(), resource_name.as_str())),
                _ => None,
            })
            .collect()
    }

    fn sort_problems(graph: &mut Graph) -> Vec<GraphBuildError> {
        match graph.sort() {
            Ok(()) => vec![],
            Err(GraphError::Invalid(problems)) => problems,
            Err(other) => panic!("expected a build error, got {other:?}"),
        }
    }

    /// A node that uses `description` as "target" with `usage`, and also writes a "link" texture
    /// so it can be connected to a later node to force a level ordering.
    fn node_using_with_link(
        name: &str,
        description: ResourceDescription,
        usage: ResourceUsage,
    ) -> GraphNode {
        node_using(name, "target", description, usage).with_resource(
            "link".to_string(),
            ResourceDescription::TestTextureResource,
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            true,
        )
    }

    /// Builds `first -> second`, where `second` reads `first`'s "link", so `first` is in an
    /// earlier level. Both use the same import as "target".
    fn two_levels_on_import(
        first_usage: ResourceUsage,
        second_usage: ResourceUsage,
    ) -> Vec<GraphBuildError> {
        let mut graph = Graph::new();
        let first = graph
            .add_node(node_using_with_link("First", swapchain(), first_usage))
            .unwrap();
        let second = graph
            .add_node(
                node_using("Second", "target", swapchain(), second_usage).with_input(
                    "link".to_string(),
                    NodeInputKind::Texture(ImageLayout::ShaderReadOnlyOptimal),
                ),
            )
            .unwrap();
        graph
            .connect(first, "link".to_string(), second, "link".to_string())
            .unwrap();
        sort_problems(&mut graph)
    }

    #[test]
    fn test_load_of_unwritten_resource_is_rejected() {
        let problems = problems_for_usage(color_load());
        assert_eq!(
            load_before_write(&problems),
            vec![("Node", "target")],
            "got {problems:?}"
        );
    }

    #[test]
    fn test_depth_load_of_unwritten_resource_is_rejected() {
        let problems = problems_for_usage(depth_load());
        assert_eq!(
            load_before_write(&problems),
            vec![("Node", "target")],
            "got {problems:?}"
        );
    }

    #[test]
    fn test_load_of_import_with_undefined_initial_layout_is_rejected() {
        // an Undefined initial layout means the contents are garbage, so there is nothing to load
        let mut graph = Graph::new();
        graph
            .add_node(node_using("Node", "target", swapchain(), color_load()))
            .unwrap();
        let problems = sort_problems(&mut graph);
        assert_eq!(
            load_before_write(&problems),
            vec![("Node", "target")],
            "got {problems:?}"
        );
    }

    #[test]
    fn test_load_of_import_with_defined_initial_layout_is_accepted() {
        // the import arrives with contents written outside the graph
        let mut graph = Graph::new();
        graph
            .add_node(node_using(
                "Node",
                "target",
                imported(
                    "history",
                    ImageLayout::ColorAttachmentOptimal,
                    ImageLayout::ColorAttachmentOptimal,
                ),
                color_load(),
            ))
            .unwrap();
        let problems = sort_problems(&mut graph);
        assert!(load_before_write(&problems).is_empty(), "got {problems:?}");
    }

    #[test]
    fn test_load_after_write_in_earlier_level_is_accepted() {
        for first_usage in [
            color_clear(ChaosClearValue::Float([0.0, 0.0, 0.0, 1.0])),
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
        ] {
            let problems = two_levels_on_import(first_usage.clone(), color_load());
            assert!(
                load_before_write(&problems).is_empty(),
                "{first_usage:?} then Load, got {problems:?}"
            );
        }
    }

    #[test]
    fn test_load_before_write_in_later_level_is_rejected() {
        let problems = two_levels_on_import(
            color_load(),
            color_clear(ChaosClearValue::Float([0.0, 0.0, 0.0, 1.0])),
        );
        assert_eq!(
            load_before_write(&problems),
            vec![("First", "target")],
            "got {problems:?}"
        );
    }

    #[test]
    fn test_load_after_load_is_rejected_for_both() {
        // a Load doesn't count as a write, so a second Load has nothing to load either
        let problems = two_levels_on_import(color_load(), color_load());
        assert_eq!(
            load_before_write(&problems),
            vec![("First", "target"), ("Second", "target")],
            "got {problems:?}"
        );
    }

    #[test]
    fn test_connector_against_add_order_on_shared_import_is_a_cycle() {
        // First is added first, so it must run before Second on the swapchain, but Second
        // feeds First through a connector
        let mut graph = Graph::new();
        let first = graph
            .add_node(
                node_using(
                    "First",
                    "target",
                    swapchain(),
                    ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                )
                .with_input(
                    "link".to_string(),
                    NodeInputKind::Texture(ImageLayout::ShaderReadOnlyOptimal),
                ),
            )
            .unwrap();
        let second = graph
            .add_node(node_using_with_link(
                "Second",
                swapchain(),
                ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            ))
            .unwrap();
        graph
            .connect(second, "link".to_string(), first, "link".to_string())
            .unwrap();

        let problems = sort_problems(&mut graph);
        assert!(
            problems.iter().any(|p| matches!(
                p,
                GraphBuildError::CircularConnection(nodes)
                    if nodes == &vec!["First".to_string(), "Second".to_string()]
            )),
            "got {problems:?}"
        );
    }

    #[test]
    fn test_node_importing_the_same_resource_twice_is_not_a_cycle() {
        // both local names share one resource id; that must not make the node its own parent
        let mut graph = Graph::new();
        graph
            .add_node(
                node_using(
                    "Node",
                    "a",
                    swapchain(),
                    ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                )
                .with_resource(
                    "b".to_string(),
                    swapchain(),
                    ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
                    true,
                ),
            )
            .unwrap();
        let problems = sort_problems(&mut graph);
        assert!(
            !problems
                .iter()
                .any(|p| matches!(p, GraphBuildError::CircularConnection(_))),
            "got {problems:?}"
        );
    }

    fn reader_of(name: &str, input: &str) -> GraphNode {
        GraphNode::new(name.to_string(), TestNode {}).with_input(
            input.to_string(),
            NodeInputKind::Texture(ImageLayout::ShaderReadOnlyOptimal),
        )
    }

    #[test]
    fn test_reader_of_import_runs_before_the_next_writer() {
        // Reader reads Source's version of the swapchain, so it has to run before Overwriter
        // replaces it, even though Overwriter was added before Reader
        let mut graph = Graph::new();
        let source = graph
            .add_node(node_writing("Source", "color", swapchain()))
            .unwrap();
        let overwriter = graph
            .add_node(node_writing("Overwriter", "color", swapchain()))
            .unwrap();
        let reader = graph.add_node(reader_of("Reader", "color")).unwrap();
        graph
            .connect(source, "color".to_string(), reader, "color".to_string())
            .unwrap();

        let (parents, children) = graph.create_hierarchy();
        assert_eq!(
            graph.compute_levels(parents, children),
            vec![vec![source], vec![reader], vec![overwriter]]
        );
    }

    #[test]
    fn test_reader_of_last_writer_gets_no_extra_edge() {
        // nothing overwrites the swapchain after Last, so Reader only depends on its connection
        let mut graph = Graph::new();
        let first = graph
            .add_node(node_writing("First", "color", swapchain()))
            .unwrap();
        let last = graph
            .add_node(node_writing("Last", "color", swapchain()))
            .unwrap();
        let reader = graph.add_node(reader_of("Reader", "color")).unwrap();
        graph
            .connect(last, "color".to_string(), reader, "color".to_string())
            .unwrap();

        let swapchain_id = graph.imported_resources["swapchain"];
        assert_eq!(
            graph.resource_order_edges(),
            vec![(first, last, swapchain_id)]
        );
    }

    #[test]
    fn test_reader_of_internal_resource_gets_no_extra_edge() {
        // a non-imported resource has a single writer, so there's never a next writer
        let mut graph = Graph::new();
        let source = graph
            .add_node(node_writing(
                "Source",
                "color",
                ResourceDescription::TestTextureResource,
            ))
            .unwrap();
        let reader = graph.add_node(reader_of("Reader", "color")).unwrap();
        graph
            .connect(source, "color".to_string(), reader, "color".to_string())
            .unwrap();

        assert!(graph.resource_order_edges().is_empty());
    }

    #[test]
    fn test_to_mermaid_draws_resource_ordering_as_dotted_edge() {
        let mut graph = Graph::new();
        let a = graph
            .add_node(node_writing("A", "color", swapchain()))
            .unwrap();
        let b = graph
            .add_node(node_writing("B", "color", swapchain()))
            .unwrap();

        let mermaid = graph.to_mermaid();
        assert!(
            mermaid.contains(&format!("node_{a} -.->|\"swapchain\"| node_{b}")),
            "{mermaid}"
        );
    }

    #[test]
    fn test_to_mermaid_skips_resource_ordering_already_shown_by_a_connection() {
        // Source -> Writer is both a connection and an add-order edge on the swapchain; only
        // the connection is drawn
        let mut graph = Graph::new();
        let source = graph
            .add_node(node_using_with_link(
                "Source",
                swapchain(),
                ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            ))
            .unwrap();
        let writer = graph
            .add_node(node_writing("Writer", "target", swapchain()).with_input(
                "link".to_string(),
                NodeInputKind::Texture(ImageLayout::ShaderReadOnlyOptimal),
            ))
            .unwrap();
        graph
            .connect(source, "link".to_string(), writer, "link".to_string())
            .unwrap();

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains(&format!("node_{source} -->")), "{mermaid}");
        assert!(!mermaid.contains("-.->"), "{mermaid}");
    }
}
