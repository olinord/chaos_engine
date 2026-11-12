use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use vulkano::{
    image::ImageLayout,
    sync::{AccessFlags, BufferMemoryBarrier, DependencyInfo, ImageMemoryBarrier, PipelineStages},
};

use crate::rendering::command_buffer::{ChaosCommandBuffer, ChaosRenderExtent};
use crate::rendering::errors::RenderError;
use crate::rendering::graph::resource::ResourceDescription;
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
    CircularConnection(Vec<String>),
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
            | GraphError::Custom(_) => None,
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

pub struct GraphContext<'a> {
    pub render_size: [u32; 2],
    pub display_size: [u32; 2],
    pub inputs: NodeResources,
    pub node_resources: NodeResources,
    pub command_buffer: &'a mut ChaosCommandBuffer,
}

impl GraphContext<'_> {
    pub fn input(&self, name: &str) -> Result<&Arc<ResourceHandle>, GraphError> {
        self.inputs.get(name)
    }

    pub fn resource(&self, name: &str) -> Result<&Arc<ResourceHandle>, GraphError> {
        self.node_resources.get(name)
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
    // jobs are sorted by levels for multithreaded rendering and async compute
    jobs: Vec<Vec<Job>>,
    sort_jobs: bool,
    recreate_resources: bool,
    render_context: Option<Arc<ChaosRenderContext>>,
    render_size: [u32; 2],
    display_size: [u32; 2],
    render_size_changed: bool,
    display_size_changed: bool,
}

impl Graph {
    pub fn new(render_size: [u32; 2]) -> Self {
        Graph {
            current_node_id: 0,
            current_resource_id: 0,
            nodes: HashMap::new(),
            connections: Vec::new(),
            resources: HashMap::new(),
            resource_lookup: HashMap::new(),
            layouts: HashMap::new(),
            jobs: Vec::new(),
            sort_jobs: true,
            recreate_resources: true,
            render_context: None,
            render_size,
            display_size: render_size,
            render_size_changed: true,
            display_size_changed: true,
        }
    }

    fn graph_context<'a>(
        &self,
        node_id: GraphNodeId,
        command_buffer: &'a mut ChaosCommandBuffer,
        inputs: &[(String, ResourceId)],
        resources: &[(String, ResourceId)],
    ) -> GraphContext<'a> {
        GraphContext {
            render_size: self.render_size,
            display_size: self.display_size,
            inputs: self.node_resources(node_id, inputs),
            node_resources: self.node_resources(node_id, resources),
            command_buffer,
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

    pub fn initialize(&mut self, render_context: Arc<ChaosRenderContext>) {
        self.render_context = Some(render_context);
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

    // nodes are moved into the graph so they don't get updated outside of the graph
    pub fn add_node(&mut self, node: GraphNode) -> GraphNodeId {
        let node_id = self.current_node_id;
        self.current_node_id += 1;
        self.sort_jobs = true;

        for (name, resource_description) in node.resource_descriptions.iter() {
            let resource_handle = ResourceHandle::new(resource_description, name);

            // the resource handle is not valid yet
            self.resources
                .insert(self.current_resource_id, Arc::new(resource_handle));
            self.resource_lookup
                .insert((node_id, name.clone()), self.current_resource_id);
            self.current_resource_id += 1;
            self.recreate_resources = true;
        }

        self.nodes.insert(node_id, Arc::new(node));

        node_id
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
        }
        problems
    }

    /// Compiles the graph into jobs. Fails with every problem `validate` and the cycle check
    /// found; once it succeeds, executing the jobs can assume a valid graph.
    fn sort(&mut self) -> Result<(), GraphError> {
        let mut problems = self.validate();

        let (parents, children) = self.create_hierarchy();
        let levels = self.compute_levels(parents, children);

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

        for level in levels {
            let mut level_jobs: Vec<Job> = vec![];
            for node_id in level {
                let node = &self.nodes[&node_id];
                let mut node_inputs: Vec<(String, ResourceId)> = Vec::new();
                // add all barriers for the current node
                for (input_name, kind) in node.input_descriptions.iter() {
                    let resource_id = *inputs
                        .get(&(node_id, input_name.clone()))
                        .expect("validation ensured this input is connected");

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

        self.sort_jobs = false;
        Ok(())
    }

    pub fn render(
        &mut self,
        destination: &Arc<ChaosTexture>,
        command_buffer: &mut ChaosCommandBuffer,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), GraphError> {
        // important to create resources before jobs
        if self.recreate_resources {
            for (resource_id, resource_handle) in self.resources.iter_mut() {
                let dependant_on_render_size = match &resource_handle.description {
                    ResourceDescription::Texture(desc) => {
                        desc.dimension_calculations.iter().any(|dim| {
                            *dim == TextureSizeType::PercentageOfRenderWidth
                                || *dim == TextureSizeType::PercentageOfRenderHeight
                        })
                    }
                    #[cfg(test)]
                    _ => false,
                };

                let dependant_on_display_size = match &resource_handle.description {
                    ResourceDescription::Texture(desc) => {
                        desc.dimension_calculations.iter().any(|dim| {
                            *dim == TextureSizeType::PercentageOfDisplayWidth
                                || *dim == TextureSizeType::PercentageOfDisplayHeight
                        })
                    }
                    #[cfg(test)]
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
        }

        if self.sort_jobs {
            self.sort()?;
        }

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
                                destination,
                                command_buffer,
                                graph_node_id,
                                node,
                                inputs,
                                outputs,
                            )?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn execute_node(
        &self,
        _destination: &Arc<ChaosTexture>,
        command_buffer: &mut ChaosCommandBuffer,
        graph_node_id: &usize,
        node: &Arc<GraphNode>,
        inputs: &[(String, ResourceId)],
        outputs: &[(String, ResourceId)],
    ) -> Result<(), GraphError> {
        let has_rendering: bool = node.queue != QueueKind::Compute && node.outputs.len() > 0;

        if has_rendering {
            // nothing will render for now, which is fine until we hook things up for realz
            let destinations = [];

            command_buffer.begin_rendering(
                &destinations,
                None,
                None,
                Some(ChaosRenderExtent {
                    size: [800, 600],
                    offset: None,
                }),
            );
        }

        // the context holds the command buffer borrow, so it is created after begin_rendering
        // and consumed by execute before end_rendering
        let graph_context = self.graph_context(*graph_node_id, command_buffer, inputs, outputs);
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
        let (src_stages, _, src_writes) = layout_scope(old_layout)
            .unwrap_or_else(|| panic!("no barrier scope for image layout {old_layout:?}"));
        let (dst_stages, dst_reads, dst_writes) = layout_scope(new_layout)
            .unwrap_or_else(|| panic!("no barrier scope for image layout {new_layout:?}"));

        let resource = resources
            .get(&resource_id)
            .and_then(|handle| handle.resource.clone())
            .expect("render creates every resource before running jobs");
        let image = match resource.as_ref() {
            Resource::Texture(image) => image.image(),
            other => panic!("image barrier on a resource that isn't a texture: {other:?}"),
        };

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
        layouts.insert(resource_id, new_layout);
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

    fn create_hierarchy(
        &self,
    ) -> (
        HashMap<GraphNodeId, Vec<GraphNodeId>>,
        HashMap<GraphNodeId, Vec<GraphNodeId>>,
    ) {
        let mut parents: HashMap<GraphNodeId, Vec<GraphNodeId>> =
            self.nodes.iter().map(|(&id, _)| (id, Vec::new())).collect();
        let mut children: HashMap<GraphNodeId, Vec<GraphNodeId>> = parents.clone();

        for connection in &self.connections {
            parents
                .entry(connection.destination_id)
                .or_default()
                .push(connection.source_id);
            children
                .entry(connection.source_id)
                .or_default()
                .push(connection.destination_id);
        }
        (parents, children)
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
        // there is no buffer resource description yet, so nothing can feed a buffer input
        (NodeInputKind::Buffer(_), _) => Some("buffer"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::graph::{
        node::NodeType,
        resource::{ResourceDescription, ResourceUsage},
    };
    use std::assert_eq;

    pub struct TestNode {}

    impl NodeType for TestNode {
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
                ResourceUsage::ColorAttachment,
                true,
            );
        }
        node
    }

    #[test]
    fn test_to_mermaid_empty_graph() {
        let graph = Graph::new([800, 600]);
        let mermaid = graph.to_mermaid();
        assert_eq!(mermaid, "```mermaid\nflowchart TD\n```");
    }

    #[test]
    fn test_to_mermaid_non_empty_graph() {
        let mut graph = Graph::new([800, 600]);
        let node_id = graph.add_node(test_node(vec![], vec![]));
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains(format!("node_{}", node_id).as_str()));
        assert!(mermaid.contains("Test Node"));
    }

    #[test]
    fn test_to_mermaid_graph_with_connection() {
        let mut graph = Graph::new([800, 600]);
        let source_id = graph.add_node(test_named_node(
            "Source Node".to_string(),
            vec![],
            vec!["source_output".to_string()],
        ));
        let destination_id = graph.add_node(test_named_node(
            "Destination Node".to_string(),
            vec!["input".to_string()],
            vec![],
        ));

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
        let mut graph = Graph::new([800, 600]);
        let root = graph.add_node(test_named_node(
            "Root".to_string(),
            vec![],
            vec!["out".to_string()],
        ));
        let left = graph.add_node(test_named_node(
            "Left".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
        let right = graph.add_node(test_named_node(
            "Right".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
        let join = graph.add_node(test_named_node(
            "Join".to_string(),
            vec!["a".to_string(), "b".to_string()],
            vec![],
        ));
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
        let mut graph = Graph::new([800, 600]);
        let gfx = graph.add_node(test_named_node("Gfx".to_string(), vec![], vec![]));
        let comp = graph.add_node(
            test_named_node("Comp".to_string(), vec![], vec![]).with_queue(QueueKind::Compute),
        );

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains(&format!("node_{}[\"Gfx\"]:::graphics", gfx)));
        assert!(mermaid.contains(&format!("node_{}[\"Comp\"]:::compute", comp)));
        assert!(mermaid.contains("classDef graphics"));
        assert!(mermaid.contains("classDef compute"));
        assert!(mermaid.contains("classDef transfer"));
    }

    #[test]
    fn test_to_mermaid_isolated_nodes_share_first_level() {
        let mut graph = Graph::new([800, 600]);
        let a = graph.add_node(test_node(vec![], vec![]));
        let b = graph.add_node(test_node(vec![], vec![]));
        let (parents, children) = graph.create_hierarchy();
        assert_eq!(graph.compute_levels(parents, children), vec![vec![a, b]]);
    }

    #[test]
    fn test_to_mermaid_marks_cycle_as_unscheduled() {
        let mut graph = Graph::new([800, 600]);
        let free = graph.add_node(test_named_node("Free".to_string(), vec![], vec![]));
        let a = graph.add_node(test_named_node(
            "A".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
        let b = graph.add_node(test_named_node(
            "B".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
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
        let mut graph = Graph::new([800, 600]);
        let ids: Vec<GraphNodeId> = (0..5)
            .map(|_| graph.add_node(test_node(vec![], vec![])))
            .collect();

        assert!(graph.sort().is_ok());
        assert_eq!(executed_node_ids_per_level(&graph), vec![ids]);
    }

    #[test]
    fn test_sort_orders_diamond_by_levels() {
        let mut graph = Graph::new([800, 600]);
        let root = graph.add_node(test_named_node(
            "Root".to_string(),
            vec![],
            vec!["out".to_string()],
        ));
        let left = graph.add_node(test_named_node(
            "Left".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
        let right = graph.add_node(test_named_node(
            "Right".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
        let join = graph.add_node(test_named_node(
            "Join".to_string(),
            vec!["a".to_string(), "b".to_string()],
            vec![],
        ));
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
        let mut graph = Graph::new([800, 600]);
        let source = graph.add_node(test_named_node(
            "Source".to_string(),
            vec![],
            vec!["source_out".to_string()],
        ));
        let sink = graph.add_node(test_named_node(
            "Sink".to_string(),
            vec!["albedo".to_string()],
            vec!["sink_out".to_string()],
        ));
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
        let mut graph = Graph::new([800, 600]);
        let node = graph.add_node(
            GraphNode::new("Node".to_string(), TestNode {})
                .with_resource(
                    "exposed".to_string(),
                    ResourceDescription::TestTextureResource,
                    ResourceUsage::ColorAttachment,
                    true,
                )
                .with_resource(
                    "scratch".to_string(),
                    ResourceDescription::TestTextureResource,
                    ResourceUsage::ColorAttachment,
                    false,
                ),
        );
        let consumer = graph.add_node(test_named_node(
            "Consumer".to_string(),
            vec!["in".to_string()],
            vec![],
        ));

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
        let mut graph = Graph::new([800, 600]);
        let source = graph.add_node(test_named_node(
            "Source".to_string(),
            vec![],
            vec!["out".to_string()],
        ));
        let sink = graph.add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
            "data".to_string(),
            NodeInputKind::Buffer(BufferUsage::StorageRead),
        ));
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
        let mut graph = Graph::new([800, 600]);
        let node_a = graph.add_node(test_named_node(
            "Node A".to_string(),
            vec!["input".to_string()],
            vec!["output".to_string()],
        ));
        let node_b = graph.add_node(test_named_node(
            "Node B".to_string(),
            vec!["input".to_string()],
            vec!["output".to_string()],
        ));
        let node_c = graph.add_node(test_named_node(
            "Node C".to_string(),
            vec!["input".to_string()],
            vec!["output".to_string()],
        ));

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
        let mut graph = Graph::new([800, 600]);
        // two unconnected inputs on one node, plus a cycle elsewhere
        graph.add_node(test_named_node(
            "Lonely".to_string(),
            vec!["a".to_string(), "b".to_string()],
            vec![],
        ));
        let x = graph.add_node(test_named_node(
            "X".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
        let y = graph.add_node(test_named_node(
            "Y".to_string(),
            vec!["in".to_string()],
            vec!["out".to_string()],
        ));
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
        let mut graph = Graph::new([800, 600]);
        let source = graph.add_node(
            GraphNode::new("Source".to_string(), TestNode {}).with_resource(
                "color".to_string(),
                texture_description(),
                ResourceUsage::ColorAttachment,
                true,
            ),
        );
        let sink = graph.add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
            "data".to_string(),
            NodeInputKind::Buffer(BufferUsage::StorageRead),
        ));
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
        let mut graph = Graph::new([800, 600]);
        let source = graph.add_node(
            GraphNode::new("Source".to_string(), TestNode {}).with_resource(
                "color".to_string(),
                texture_description(),
                ResourceUsage::ColorAttachment,
                true,
            ),
        );
        let sink = graph.add_node(GraphNode::new("Sink".to_string(), TestNode {}).with_input(
            "color".to_string(),
            NodeInputKind::Texture(ImageLayout::Undefined),
        ));
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
        let mut graph = Graph::new([800, 600]);
        let node_id = graph.add_node(test_node(
            vec!["input".to_string()],
            vec!["output".to_string()],
        ));
        let result = graph.connect(node_id, "output".to_string(), node_id, "input".to_string());
        assert!(
            result.is_err(),
            "Expected connecting to the same node to return an error"
        );
    }
}
