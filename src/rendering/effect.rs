use std::{collections::HashMap, sync::Arc};

use vulkano::{buffer::BufferContents, pipeline::GraphicsPipeline};

use crate::rendering::buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage};
use crate::rendering::rendering_system::ChaosRenderContext;

pub struct ChaosEffect {
    pub name: String,
    pipeline: Arc<GraphicsPipeline>,
    buffers: HashMap<u32, ChaosBuffer>,
    render_context: Arc<ChaosRenderContext>,
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
            buffers: HashMap::new(),
            render_context,
        }
    }

    pub fn set_data<T: BufferContents>(&mut self, layout_index: u32, data: Vec<T>) {
        match self.buffers.get_mut(&layout_index) {
            Some(existing_buffer) => match existing_buffer.set_data(data) {
                Ok(()) => {}
                Err(error) => {
                    panic!(
                        "Failed to set data for buffer {}: {}",
                        existing_buffer.name, error
                    );
                }
            },
            None => {
                let mut new_buffer = ChaosBuffer::new(
                    format!("{}-buffer-{}", self.name, layout_index),
                    ChaosBufferUsage::UniformBuffer,
                    ChaosBufferMemoryType::PreferDevice,
                    self.render_context.clone(),
                );
                match new_buffer.set_data(data) {
                    Ok(()) => {
                        self.buffers.insert(layout_index, new_buffer);
                    }
                    Err(error) => {
                        panic!(
                            "Failed to set data for buffer {}: {}",
                            new_buffer.name, error
                        );
                    }
                }
            }
        }
    }

    pub fn pipeline(&self) -> Arc<GraphicsPipeline> {
        self.pipeline.clone()
    }
}
