use std::hash::{DefaultHasher, Hash, Hasher};
use std::{collections::HashMap, sync::Arc};

use vulkano::ValidationError;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::{
    StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::pipeline::{Pipeline, PipelineBindPoint};
use vulkano::{buffer::BufferContents, pipeline::GraphicsPipeline};

use crate::rendering::buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage};
use crate::rendering::renderer::ChaosRenderContext;

type DescriptorBinding = (u32, u32);

pub struct ChaosEffect {
    pub name: String,
    pub hash: u64,
    pipeline: Arc<GraphicsPipeline>,
    render_context: Arc<ChaosRenderContext>,
    pipeline_bind_point: PipelineBindPoint,
    uniform_buffers: HashMap<DescriptorBinding, ChaosBuffer>,
    storage_buffers: HashMap<DescriptorBinding, ChaosBuffer>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
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
            descriptor_set_allocator: Arc::new(StandardDescriptorSetAllocator::new(
                render_context.device(),
                StandardDescriptorSetAllocatorCreateInfo::default(),
            )),
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
        let buffer = match self.uniform_buffers.get_mut(&(set_index, binding_index)) {
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

                self.uniform_buffers
                    .insert((set_index, binding_index), new_buffer);

                self.hash = self.sort_key();
                self.uniform_buffers
                    .get_mut(&(set_index, binding_index))
                    .unwrap()
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
        let buffer = match self.uniform_buffers.get_mut(&(set_index, binding_index)) {
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

                self.uniform_buffers
                    .insert((set_index, binding_index), new_buffer);

                self.hash = self.sort_key();
                self.uniform_buffers
                    .get_mut(&(set_index, binding_index))
                    .unwrap()
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
        let buffer = match self.storage_buffers.get_mut(&(set_index, binding_index)) {
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

                self.storage_buffers
                    .insert((set_index, binding_index), new_buffer);

                self.hash = self.sort_key();
                self.storage_buffers
                    .get_mut(&(set_index, binding_index))
                    .unwrap()
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
        let buffer = match self.storage_buffers.get_mut(&(set_index, binding_index)) {
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

                self.storage_buffers
                    .insert((set_index, binding_index), new_buffer);

                self.hash = self.sort_key();
                self.storage_buffers
                    .get_mut(&(set_index, binding_index))
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

    pub fn bind_descriptor_sets(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), Box<ValidationError>> {
        let pipeline_layout = self.pipeline.layout().clone();

        for (set_index, set_layout) in pipeline_layout.set_layouts().iter().enumerate() {
            let descriptor_writes = self.descriptor_writes_for_set(set_index as u32);
            if descriptor_writes.is_empty() {
                continue;
            }

            let descriptor_set = DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                set_layout.clone(),
                descriptor_writes,
                std::iter::empty(),
            )
            .map_err(|error| {
                Box::new(ValidationError {
                    context: "ChaosEffect::bind_descriptor_sets".into(),
                    problem: format!(
                        "failed to allocate descriptor set {} for effect {}: {:?}",
                        set_index, self.name, error
                    )
                    .into(),
                    ..Default::default()
                })
            })?;

            command_buffer.bind_descriptor_sets(
                self.pipeline_bind_point,
                pipeline_layout.clone(),
                set_index as u32,
                descriptor_set,
            )?;
        }

        Ok(())
    }

    fn descriptor_writes_for_set(&self, set_index: u32) -> Vec<WriteDescriptorSet> {
        let mut descriptor_writes = Vec::new();
        self.add_descriptor_writes_for_set(
            set_index,
            &self.uniform_buffers,
            &mut descriptor_writes,
        );
        self.add_descriptor_writes_for_set(
            set_index,
            &self.storage_buffers,
            &mut descriptor_writes,
        );
        descriptor_writes.sort_by_key(|write| write.binding());
        descriptor_writes
    }

    fn add_descriptor_writes_for_set(
        &self,
        set_index: u32,
        buffers: &HashMap<DescriptorBinding, ChaosBuffer>,
        descriptor_writes: &mut Vec<WriteDescriptorSet>,
    ) {
        for ((buffer_set_index, binding_index), buffer) in buffers {
            if *buffer_set_index != set_index {
                continue;
            }

            if let Some(buffer) = buffer.buffer() {
                descriptor_writes.push(WriteDescriptorSet::buffer(
                    *binding_index,
                    buffer.as_ref().clone(),
                ));
            }
        }
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
