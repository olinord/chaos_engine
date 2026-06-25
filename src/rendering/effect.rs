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
use crate::rendering::rendering_system::ChaosRenderContext;

type DescriptorBinding = (u32, u32);

pub struct ChaosEffect {
    pub name: String,
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
        data: Vec<T>,
    ) -> Result<(), String> {
        let mut buffer = ChaosBuffer::new(
            format!(
                "{}-uniform-buffer-{}-{}",
                self.name, set_index, binding_index
            ),
            ChaosBufferUsage::UniformBuffer,
            ChaosBufferMemoryType::PreferHost,
            self.render_context.clone(),
        );
        buffer.set_data(data)?;
        self.set_uniform_buffer(set_index, binding_index, buffer);
        Ok(())
    }

    pub fn set_uniform_buffer(&mut self, set_index: u32, binding_index: u32, buffer: ChaosBuffer) {
        self.storage_buffers.remove(&(set_index, binding_index));
        self.uniform_buffers
            .insert((set_index, binding_index), buffer);
    }

    pub fn set_storage_data<T: BufferContents>(
        &mut self,
        set_index: u32,
        binding_index: u32,
        data: Vec<T>,
    ) -> Result<(), String> {
        let mut buffer = ChaosBuffer::new(
            format!(
                "{}-storage-buffer-{}-{}",
                self.name, set_index, binding_index
            ),
            ChaosBufferUsage::StorageBuffer,
            ChaosBufferMemoryType::PreferHost,
            self.render_context.clone(),
        );
        buffer.set_data(data)?;
        self.set_storage_buffer(set_index, binding_index, buffer);
        Ok(())
    }

    pub fn set_storage_buffer(&mut self, set_index: u32, binding_index: u32, buffer: ChaosBuffer) {
        self.uniform_buffers.remove(&(set_index, binding_index));
        self.storage_buffers
            .insert((set_index, binding_index), buffer);
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
}
