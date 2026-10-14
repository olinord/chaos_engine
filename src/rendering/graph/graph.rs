use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use vulkano::{
    Validated, ValidationError, VulkanError,
    command_buffer::{
        CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer,
        RenderPassBeginInfo, RenderingInfo, SubpassEndInfo,
        allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo},
    },
    image::ImageLayout,
    render_pass::RenderPass,
    sync::{DependencyInfo, ImageMemoryBarrier},
};

use crate::rendering::{
    context::ChaosRenderContext,
    graph::{
        node::{self, Connector, GraphNode, GraphNodeId, NodeInputKind},
        resource::{Resource, ResourceDescription, ResourceHandle},
    },
    texture::ChaosTexture,
};

pub type ResourceId = usize;
pub const DESTINATION_RESOURCE_ID: ResourceId = 0;

#[derive(Debug, PartialEq, Eq)]
pub enum GraphError {
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
    ResourceNotFound {
        resource_id: ResourceId,
    },
    ConnectionBetweenSameNode {
        node_id: GraphNodeId,
    },
    CircularConnection,
    BarrierError(String),
    CommandBufferCreationFailed(String),
    NodeExecutionFailed(String),
    VulkanError(String),
    Custom(String),
}

impl From<Validated<VulkanError>> for GraphError {
    fn from(value: Validated<VulkanError>) -> Self {
        GraphError::VulkanError(value.to_string())
    }
}
impl From<Box<ValidationError>> for GraphError {
    fn from(value: Box<ValidationError>) -> Self {
        GraphError::VulkanError(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum Job {
    BarrierJob {
        resource_id: ResourceId,
        image_layout: ImageLayout,
    },
    NodeExecutionJob {
        graph_node_id: GraphNodeId,
    },
}

#[derive(Debug)]
pub struct GraphContext<'a> {
    pub render_size: [u32; 2],
    pub display_size: [u32; 2],
    pub resources: HashMap<ResourceId, Arc<ResourceHandle>>,
    pub resource_lookup: HashMap<String, ResourceId>,
    pub command_buffer: &'a mut RecordingCommandBuffer,
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
    // jobs are sorted by levels for multithreaded rendering and async compute
    jobs: Vec<Vec<Job>>,
    recreate_jobs: bool,
    recreate_resources: bool,
    render_context: Option<Arc<ChaosRenderContext>>,
    command_buffer_allocator: Option<Arc<StandardCommandBufferAllocator>>,
    render_size: [u32; 2],
    display_size: [u32; 2],
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
            jobs: Vec::new(),
            recreate_jobs: true,
            recreate_resources: true,
            render_context: None,
            command_buffer_allocator: None,
            render_size,
            display_size: render_size,
        }
    }

    fn graph_context<'a>(
        &self,
        graph_node_id: GraphNodeId,
        command_buffer: &'a mut RecordingCommandBuffer,
        destination: &Arc<ChaosTexture>,
    ) -> GraphContext<'a> {
        let resource_lookup = self
            .resource_lookup
            .iter()
            .filter(|((id, _), _)| *id == graph_node_id)
            .map(|((_, name), resource_id)| (name.clone(), *resource_id))
            .collect::<HashMap<String, ResourceId>>();
        let mut resources = resource_lookup
            .iter()
            .filter_map(|(_, resource_id)| {
                self.resources
                    .get(resource_id)
                    .map(|resource| (*resource_id, resource.clone()))
            })
            .collect::<HashMap<ResourceId, Arc<ResourceHandle>>>();

        GraphContext {
            render_size: self.render_size,
            display_size: self.display_size,
            resources,
            resource_lookup,
            command_buffer,
        }
    }

    pub fn initialize(&mut self, render_context: Arc<ChaosRenderContext>) {
        self.render_context = Some(render_context);

        self.command_buffer_allocator = Some(Arc::new(StandardCommandBufferAllocator::new(
            self.render_context.as_ref().unwrap().device().clone(),
            StandardCommandBufferAllocatorCreateInfo::default(),
        )));
    }

    pub fn set_render_size(&mut self, render_size: [u32; 2]) {
        self.render_size = render_size;
        self.recreate_resources = true;
    }

    pub fn set_display_size(&mut self, display_size: [u32; 2]) {
        self.display_size = display_size;
        self.recreate_resources = true;
    }

    // nodes are moved into the graph so they don't get updated outside of the graph
    pub fn add_node(&mut self, node: GraphNode) -> GraphNodeId {
        let node_id = self.current_node_id;
        self.current_node_id += 1;
        self.recreate_jobs = true;
        self.recreate_resources = self.recreate_resources || node.resource_descriptions.len() > 0;
        self.nodes.insert(node_id, Arc::new(node));

        node_id
    }

    pub fn connect(
        &mut self,
        source: GraphNodeId,
        source_resource: String,
        destination: GraphNodeId,
        destination_input: String,
    ) -> Result<(), GraphError> {
        if source == destination {
            return Err(GraphError::ConnectionBetweenSameNode { node_id: source });
        }

        // check if the source exists
        match self.nodes.get(&source) {
            Some(source_node) => {
                // source node needs to contain the specified resource
                if !source_node.outputs.contains(&source_resource) {
                    return Err(GraphError::SourceResourceNotFound {
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
                            return Err(GraphError::DestinationInputNotFound {
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
                        self.recreate_jobs = true;

                        return Ok(());
                    }
                    None => {
                        return Err(GraphError::DestinationNodeNotFound {
                            node_id: destination,
                        });
                    }
                }
            }
            None => {
                return Err(GraphError::SourceNodeNotFound { node_id: source });
            }
        }
    }

    fn sort(&mut self) -> Result<(), GraphError> {
        self.jobs.clear();

        let mut in_degree: HashMap<GraphNodeId, usize> = HashMap::new();
        let mut successors: HashMap<GraphNodeId, Vec<GraphNodeId>> = HashMap::new();
        for connection in self.connections.iter() {
            *in_degree.entry(connection.destination_id).or_insert(0) += 1;
            successors
                .entry(connection.source_id)
                .or_insert_with(Vec::new)
                .push(connection.destination_id);
        }

        let mut ready_levels: Vec<Vec<GraphNodeId>> = vec![
            in_degree
                .iter()
                .filter(|(_, &deg)| deg == 0)
                .map(|(&node_id, _)| node_id)
                .collect(),
        ];

        let mut order: Vec<GraphNodeId> = Vec::new();

        while !ready_levels.is_empty() {
            if let Some(mut level) = ready_levels.pop() {
                let mut next_level = vec![];
                let mut level_jobs: Vec<Job> = vec![];
                while let Some(node_id) = level.pop() {
                    let node = self.nodes.get(&node_id).unwrap();
                    order.push(node_id);
                    // add all barriers for the current node
                    for (input_name, kind) in node.input_descriptions.iter() {
                        // add a barrier
                        level_jobs.push(Job::BarrierJob {
                            resource_id: self
                                .resource_lookup
                                .get(&(node_id, input_name.clone()))
                                .unwrap()
                                .clone(),
                            image_layout: match kind {
                                NodeInputKind::Texture(layout) => *layout,
                                _ => ImageLayout::Undefined,
                            },
                        });
                    }
                    level_jobs.push(Job::NodeExecutionJob {
                        graph_node_id: node_id,
                    });
                    if let Some(succs) = successors.get(&node_id) {
                        for &succ in succs {
                            if let Some(deg) = in_degree.get_mut(&succ) {
                                *deg -= 1;
                                if *deg == 0 {
                                    next_level.push(succ);
                                }
                            }
                        }
                    }
                }
                if !next_level.is_empty() {
                    ready_levels.push(next_level);
                }
                self.jobs.push(level_jobs);
            }
        }

        if order.len() != self.nodes.len() {
            return Err(GraphError::CircularConnection);
        }

        // self
        //     .connections
        //     .clone()
        //     .into_iter()
        //     .fold(HashMap::new(), |mut acc, connection| {
        //         acc.entry(connection.destination_id)
        //             .or_insert_with(Vec::new)
        //             .push(connection.source_id);
        //         acc
        //     });

        // while !dependency_tree.is_empty() {
        //     // process nodes with no dependencies
        //     let ready_nodes: Vec<GraphNodeId> = dependency_tree
        //         .iter()
        //         .filter(|(_, deps)| deps.is_empty())
        //         .map(|(&node_id, _)| node_id)
        //         .collect();

        //     if ready_nodes.is_empty() {
        //         // Circular dependency detected
        //         break;
        //     }

        //     for node_id in ready_nodes {
        //         // if the node requires inputs then add a barrier for those things
        //         let node = &self.nodes[&node_id];
        //         for (input_name, kind) in node.input_descriptions.iter() {
        //             // add a barrier
        //             self.jobs.push(Job::BarrierJob {
        //                 resource_id: self
        //                     .resource_lookup
        //                     .get(&(node_id, input_name.clone()))
        //                     .unwrap()
        //                     .clone(),
        //                 image_layout: match kind {
        //                     NodeInputKind::Texture(layout) => *layout,
        //                     _ => ImageLayout::Undefined,
        //                 },
        //             });
        //         }
        //         self.jobs.push(Job::NodeExecutionJob {
        //             graph_node_id: node_id,
        //         });
        //         dependency_tree.remove(&node_id);
        //         for deps in dependency_tree.values_mut() {
        //             deps.retain(|&dep| dep != node_id);
        //         }
        //     }
        // }

        // if !dependency_tree.is_empty() {
        //     Err(GraphError::CircularConnection)
        // } else {
        //     Ok(())
        // }
        Ok(())
    }

    fn create_resources(
        &mut self,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), GraphError> {
        self.resources.clear();

        for (node_id, node) in &self.nodes {
            for (name, resource_description) in node.resource_descriptions.iter() {
                let mut resource_handle = ResourceHandle::new(resource_description, name);
                resource_handle.recreate_resource(
                    self.display_size,
                    self.render_size,
                    render_context.clone(),
                );
                self.resources
                    .insert(self.current_resource_id, Arc::new(resource_handle));
                self.resource_lookup
                    .insert((*node_id, name.clone()), self.current_resource_id);
                self.current_resource_id += 1;
            }
        }

        Ok(())
    }

    pub fn render(
        &mut self,
        destination: &Arc<ChaosTexture>,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), GraphError> {
        // important to create resources before jobs
        if self.recreate_resources {
            self.create_resources(render_context)?;
            self.recreate_resources = false;
        }

        if self.recreate_jobs {
            self.sort()?;
            self.recreate_jobs = false;
        }

        if self.command_buffer_allocator.is_none() {
            self.initialize(render_context.clone());
        }

        let mut command_buffer = RecordingCommandBuffer::new(
            self.command_buffer_allocator.as_ref().unwrap().clone(),
            render_context.graphic_family_index(),
            CommandBufferLevel::Primary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                ..Default::default()
            },
        )?;

        for level in self.jobs.clone() {
            for job in &level {
                match job {
                    Job::BarrierJob { .. } => {
                        self.add_resource_barrier(&job, &mut command_buffer)?
                    }
                    Job::NodeExecutionJob { graph_node_id } => {
                        if let Some(node) = self.nodes.get(graph_node_id) {
                            self.execute_graph(
                                destination,
                                &mut command_buffer,
                                graph_node_id,
                                node,
                            )?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn execute_graph(
        &self,
        destination: &Arc<ChaosTexture>,
        command_buffer: &mut RecordingCommandBuffer,
        graph_node_id: &usize,
        node: &Arc<GraphNode>,
    ) -> Result<(), GraphError> {
        let graph_context = self.graph_context(*graph_node_id, command_buffer, &destination);

        let rendering_info = RenderingInfo {
            // Fill in the necessary fields for the render pass begin info
            ..Default::default()
        };
        unsafe {
            command_buffer.begin_rendering(&rendering_info)?;
        };

        node.node_type.execute(graph_context)?;

        unsafe {
            command_buffer.end_rendering()?;
        }
        Ok(())
    }

    fn add_resource_barrier(
        &mut self,
        job: &Job,
        command_buffer: &mut RecordingCommandBuffer,
    ) -> Result<(), GraphError> {
        if let Job::BarrierJob {
            image_layout,
            resource_id,
        } = job
        {
            match self.resources.get_mut(resource_id).and_then(Arc::get_mut) {
                Some(resource_handle) => {
                    match resource_handle.resource.as_mut().and_then(Arc::get_mut) {
                        Some(Resource::Texture(image)) => {
                            let mut barrier = ImageMemoryBarrier::image(image.image());
                            barrier.old_layout = image.get_image_layout();
                            barrier.new_layout = image_layout.clone();
                            unsafe {
                                command_buffer.pipeline_barrier(&DependencyInfo {
                                    image_memory_barriers: vec![barrier].into(),
                                    ..Default::default()
                                })?;
                            }
                            if let Some(image) = Arc::get_mut(image) {
                                image.set_image_layout(image_layout.clone());
                            }
                            Ok(())
                        }
                        _ => Err(GraphError::BarrierError(
                            "Resource is not a texture".to_string(),
                        )),
                    }
                }
                _ => Err(GraphError::BarrierError("Resource not found".to_string())),
            }
        } else {
            Err(GraphError::BarrierError(
                "Job is not a barrier job".to_string(),
            ))
        }
    }

    /// Groups nodes into dependency levels using Kahn's algorithm: every node in a level only
    /// depends on nodes from earlier levels, so nodes within a level can run in parallel.
    /// Nodes that are part of (or downstream of) a cycle never reach in-degree 0 and are
    /// left out of the result. Each level is sorted by node id to keep the output stable.
    fn compute_levels(&self) -> Vec<Vec<GraphNodeId>> {
        let mut in_degree: HashMap<GraphNodeId, usize> =
            self.nodes.keys().map(|&node_id| (node_id, 0)).collect();
        let mut successors: HashMap<GraphNodeId, Vec<GraphNodeId>> = HashMap::new();
        for connection in &self.connections {
            *in_degree.entry(connection.destination_id).or_insert(0) += 1;
            successors
                .entry(connection.source_id)
                .or_default()
                .push(connection.destination_id);
        }

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
                for &successor in successors.get(node_id).into_iter().flatten() {
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

    /// Builds a Mermaid flowchart representation of the graph: one box per node and one
    /// labeled edge per connection, showing which output resource feeds which input.
    /// Nodes are grouped into one subgraph per dependency level; nodes in the same level
    /// have no dependency on each other and can be parallelized. Nodes that can't be
    /// scheduled because of a cycle are shown in a separate "Unscheduled" subgraph.
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("```mermaid\nflowchart TD\n");

        let levels = self.compute_levels();
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
            let label = if level.len() > 1 {
                format!("Level {} (can run in parallel)", index)
            } else {
                format!("Level {}", index)
            };
            self.push_mermaid_subgraph(&mut mermaid, &format!("level_{}", index), &label, level);
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
            let resource_name = self
                .nodes
                .get(&connection.source_id)
                .unwrap()
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
            mermaid.push_str(&format!(
                "        node_{}[\"{}\"]\n",
                node_id, self.nodes[node_id].name
            ));
        }
        mermaid.push_str("    end\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::graph::{node::NodeType, resource::ResourceDescription};
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
            node = node.with_resource(output, ResourceDescription::TestTextureResource, true);
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
        let source_id = graph.add_node(test_node(vec![], vec!["source_output".to_string()]));
        let destination_id = graph.add_node(test_node(vec!["input".to_string()], vec![]));

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

        assert_eq!(
            graph.compute_levels(),
            vec![vec![root], vec![left, right], vec![join]]
        );

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("subgraph level_0[\"Level 0\"]"));
        assert!(mermaid.contains("subgraph level_1[\"Level 1 (can run in parallel)\"]"));
        assert!(mermaid.contains("subgraph level_2[\"Level 2\"]"));
    }

    #[test]
    fn test_to_mermaid_isolated_nodes_share_first_level() {
        let mut graph = Graph::new([800, 600]);
        let a = graph.add_node(test_node(vec![], vec![]));
        let b = graph.add_node(test_node(vec![], vec![]));
        assert_eq!(graph.compute_levels(), vec![vec![a, b]]);
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

        assert_eq!(graph.compute_levels(), vec![vec![free]]);
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("subgraph unscheduled"));
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
        assert_eq!(
            result.err().unwrap(),
            GraphError::CircularConnection,
            "Expected circular connection to be detected",
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
