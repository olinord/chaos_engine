use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};
use vulkano::{Validated, ValidationError, VulkanError};
use vulkano::{buffer::BufferContents, pipeline::GraphicsPipeline};

use crate::rendering::buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage};
use crate::rendering::context::ChaosRenderContext;

#[derive(Hash, Eq, PartialEq, Debug, Clone, PartialOrd)]
struct DescriptorBinding {
    set_index: u32,
    binding_index: u32,
}

impl Ord for DescriptorBinding {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.set_index.cmp(&other.set_index) {
            Ordering::Equal => self.binding_index.cmp(&other.binding_index),
            other => other,
        }
    }
}

pub struct ChaosEffect {
    pub name: String,
    pub hash: u64,
    pipeline: Arc<GraphicsPipeline>,
    render_context: Arc<ChaosRenderContext>,
    pipeline_bind_point: PipelineBindPoint,
    uniform_buffers: HashMap<DescriptorBinding, ChaosBuffer>,
    storage_buffers: HashMap<DescriptorBinding, ChaosBuffer>,
}

impl ChaosEffect {
    pub fn new(
        name: &str,
        pipeline: Arc<GraphicsPipeline>,
        render_context: Arc<ChaosRenderContext>,
    ) -> Self {
        Self {
            name: name.to_string(),
            hash: 0,
            pipeline,
            render_context: render_context.clone(),
            pipeline_bind_point: PipelineBindPoint::Graphics,
            uniform_buffers: HashMap::new(),
            storage_buffers: HashMap::new(),
        }
    }

    pub fn set_pipeline_bind_point(&mut self, bind_point: PipelineBindPoint) {
        self.pipeline_bind_point = bind_point;
    }

    pub fn set_uniform_data<T: BufferContents>(
        &mut self,
        set_index: u32,
        binding_index: u32,
        data: T,
    ) -> Result<(), String> {
        let binding = DescriptorBinding {
            set_index,
            binding_index,
        };
        let buffer = match self.uniform_buffers.get_mut(&binding) {
            Some(buffer) => buffer,
            None => {
                let new_buffer = ChaosBuffer::new(
                    format!(
                        "{}-uniform-buffer-{}-{}",
                        self.name, set_index, binding_index
                    ),
                    ChaosBufferUsage::UniformBuffer,
                    ChaosBufferMemoryType::PreferHost,
                    self.render_context.clone(),
                );

                self.uniform_buffers.insert(binding.clone(), new_buffer);

                self.hash = self.sort_key();
                self.uniform_buffers.get_mut(&binding).unwrap()
            }
        };
        buffer.set_data(data)?;
        Ok(())
    }

    pub fn set_uniform_data_vec<T: BufferContents>(
        &mut self,
        set_index: u32,
        binding_index: u32,
        data: Vec<T>,
    ) -> Result<(), String> {
        let binding = DescriptorBinding {
            set_index,
            binding_index,
        };
        let buffer = match self.uniform_buffers.get_mut(&binding) {
            Some(buffer) => buffer,
            None => {
                let new_buffer = ChaosBuffer::new(
                    format!(
                        "{}-uniform-buffer-{}-{}",
                        self.name, set_index, binding_index
                    ),
                    ChaosBufferUsage::UniformBuffer,
                    ChaosBufferMemoryType::PreferHost,
                    self.render_context.clone(),
                );

                self.uniform_buffers.insert(binding.clone(), new_buffer);

                self.hash = self.sort_key();
                self.uniform_buffers.get_mut(&binding).unwrap()
            }
        };
        buffer.set_data_from_vec(data)?;
        Ok(())
    }

    pub fn set_storage_data<T: BufferContents>(
        &mut self,
        set_index: u32,
        binding_index: u32,
        data: T,
    ) -> Result<(), String> {
        let binding = DescriptorBinding {
            set_index,
            binding_index,
        };
        let buffer = match self.storage_buffers.get_mut(&binding) {
            Some(buffer) => buffer,
            None => {
                let new_buffer = ChaosBuffer::new(
                    format!(
                        "{}-storage-buffer-{}-{}",
                        self.name, set_index, binding_index
                    ),
                    ChaosBufferUsage::StorageBuffer,
                    ChaosBufferMemoryType::PreferHost,
                    self.render_context.clone(),
                );

                self.storage_buffers.insert(binding.clone(), new_buffer);

                self.hash = self.sort_key();
                self.storage_buffers.get_mut(&binding).unwrap()
            }
        };
        buffer.set_data(data)?;
        Ok(())
    }

    pub fn set_storage_data_vec<T: BufferContents>(
        &mut self,
        set_index: u32,
        binding_index: u32,
        data: Vec<T>,
    ) -> Result<(), String> {
        let buffer = match self.storage_buffers.get_mut(&DescriptorBinding {
            set_index,
            binding_index,
        }) {
            Some(buffer) => buffer,
            None => {
                let new_buffer = ChaosBuffer::new(
                    format!(
                        "{}-storage-buffer-{}-{}",
                        self.name, set_index, binding_index
                    ),
                    ChaosBufferUsage::StorageBuffer,
                    ChaosBufferMemoryType::PreferHost,
                    self.render_context.clone(),
                );

                self.storage_buffers.insert(
                    DescriptorBinding {
                        set_index,
                        binding_index,
                    },
                    new_buffer,
                );

                self.hash = self.sort_key();
                self.storage_buffers
                    .get_mut(&DescriptorBinding {
                        set_index,
                        binding_index,
                    })
                    .unwrap()
            }
        };
        buffer.set_data_from_vec(data)?;
        Ok(())
    }

    pub fn bind_push_constants<T: BufferContents>(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        offset: u32,
        data: T,
    ) -> Result<(), Box<ValidationError>> {
        command_buffer.push_constants(self.pipeline.layout().clone(), offset, data)?;
        Ok(())
    }

    fn get_descriptor_writes(&self, set_index: u32) -> Vec<WriteDescriptorSet> {
        let mut descriptor_writes = Vec::new();

        let add =
            |binding_index: u32, buffer: &ChaosBuffer, writes: &mut Vec<WriteDescriptorSet>| {
                if let Some(buffer) = buffer.buffer() {
                    writes.push(WriteDescriptorSet::buffer(
                        binding_index,
                        buffer.as_ref().clone(),
                    ));
                }
            };

        let add_buffers = |set_index: u32,
                           buffers: &HashMap<DescriptorBinding, ChaosBuffer>,
                           writes: &mut Vec<WriteDescriptorSet>| {
            for (binding, buffer) in buffers {
                if binding.set_index == set_index {
                    add(binding.binding_index, buffer, writes);
                }
            }
        };

        add_buffers(set_index, &self.uniform_buffers, &mut descriptor_writes);
        add_buffers(set_index, &self.storage_buffers, &mut descriptor_writes);

        descriptor_writes
    }

    pub fn pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }

    /// Content-based hash of the GPU state this effect will bind: the pipeline
    /// pointer plus every buffer bound to a descriptor set slot. Two effects
    /// that would issue identical `bind_pipeline` + `bind_descriptor_sets`
    /// calls produce the same key, so they sort adjacently in the draw queue
    /// and can be batched.
    ///
    /// Stable across frames as long as the underlying `Arc`s aren't
    /// reallocated (i.e. bindings aren't reassigned). Not stable across
    /// process runs (pointer values are non-deterministic).
    fn sort_key(&self) -> u64 {
        const UNIFORM_TAG: u8 = 0;
        const STORAGE_TAG: u8 = 1;

        let mut hasher = DefaultHasher::new();

        Arc::as_ptr(&self.pipeline).hash(&mut hasher);
        (self.pipeline_bind_point as u32).hash(&mut hasher);

        hash_buffer_bindings(&mut hasher, UNIFORM_TAG, &self.uniform_buffers);
        hash_buffer_bindings(&mut hasher, STORAGE_TAG, &self.storage_buffers);

        hasher.finish()
    }

    // goes through the descriptor writes for the pipeline layout
    // gets or creates the desciptor set from the render context and binds it to the command buffer.
    pub fn bind_descriptor_sets(
        &self,
        cb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), Validated<VulkanError>> {
        let layout = self.pipeline.layout().clone();

        for (set_index, set_layout) in layout.set_layouts().iter().enumerate() {
            let set_index = set_index as u32;
            let buffers_for_set = self
                .storage_buffers
                .iter()
                .chain(self.uniform_buffers.iter())
                .filter(|(binding, buffer)| {
                    (binding.set_index == set_index) && buffer.buffer().is_some()
                })
                .map(|(binding, buffer)| {
                    WriteDescriptorSet::buffer(
                        binding.binding_index,
                        buffer.buffer().unwrap().as_ref().clone(),
                    )
                })
                .collect::<Vec<_>>();

            if buffers_for_set.is_empty() {
                continue;
            }
            let writes = self.get_descriptor_writes(set_index);

            let key = self.descriptor_set_key(set_index);
            let set = self
                .render_context
                .get_or_create_descriptor_set(key, set_layout, || writes)
                .map_err(|e| {
                    Box::new(ValidationError {
                        context: "ChaosEffect::bind_descriptor_sets".into(),
                        problem: format!("set {set_index} for {}: {e:?}", self.name).into(),
                        ..Default::default()
                    })
                })?;
            cb.bind_descriptor_sets(self.pipeline_bind_point, layout.clone(), set_index, set)?;
        }

        Ok(())
    }

    fn descriptor_set_key(&self, set_index: u32) -> u64 {
        let mut hasher = DefaultHasher::new();
        set_index.hash(&mut hasher);
        Arc::as_ptr(&self.pipeline).hash(&mut hasher);
        hasher.finish()
    }
}

fn hash_buffer_bindings(
    hasher: &mut DefaultHasher,
    tag: u8,
    buffers: &HashMap<DescriptorBinding, ChaosBuffer>,
) {
    // HashMap iteration order is nondeterministic; sort keys for a stable hash.
    let mut keys: Vec<&DescriptorBinding> = buffers.keys().collect();
    keys.sort();

    for key in keys {
        tag.hash(hasher);
        key.hash(hasher);
        if let Some(buffer) = buffers[key].buffer() {
            Arc::as_ptr(&buffer).hash(hasher);
        }
    }
}
