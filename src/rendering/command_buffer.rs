use std::any::Any;
use std::debug_assert;
use std::sync::Arc;

use vulkano::buffer::{BufferContents, IndexBuffer, Subbuffer};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    CommandBuffer, DrawIndexedIndirectCommand, RenderingAttachmentInfo, RenderingInfo,
};
use vulkano::descriptor_set::DescriptorSet;
use vulkano::descriptor_set::sys::RawDescriptorSet;
use vulkano::format::ClearValue;
use vulkano::image::ImageLayout;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::{ComputePipeline, GraphicsPipeline};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::DependencyInfo;
use vulkano::{
    command_buffer::{
        CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsage, RecordingCommandBuffer,
    },
    pipeline::{PipelineBindPoint, PipelineLayout, graphics::viewport::Scissor},
};

use crate::rendering::errors::ChaosRenderingError;
use crate::rendering::texture::ChaosTexture;

pub struct ChaosCommandBuffer {
    cmd_buffer: RecordingCommandBuffer,
    inside_rendering: bool,
    keep_alive: Vec<Arc<dyn Any + Send + Sync>>,
}

pub struct FinishedFrame {
    pub cmd_buffer: Arc<CommandBuffer>,
    keep_alive: Vec<Arc<dyn Any + Send + Sync>>,
}

pub enum ChaosClearValue {
    Float([f32; 4]),
    Int([i32; 4]),
    Uint([u32; 4]),
    Depth(f32),
    Stencil(u32),
    DepthStencil(f32, u32),
}

pub enum ChaosRenderAttachmentLoadOp {
    Load,
    Clear(ChaosClearValue),
    DontCare,
}

pub struct ChaosRenderExtent {
    pub size: [u32; 2],
    pub offset: Option<[u32; 2]>,
}

pub struct ChaosRenderAttachment {
    texture: Arc<ChaosTexture>,
    load_op: ChaosRenderAttachmentLoadOp,
    layout: ImageLayout,
    store: bool,
}

impl ChaosRenderAttachment {
    pub fn new(
        texture: Arc<ChaosTexture>,
        load_op: ChaosRenderAttachmentLoadOp,
        layout: ImageLayout,
        store: bool,
    ) -> Self {
        Self {
            texture,
            load_op,
            layout,
            store,
        }
    }
}

impl Into<RenderingAttachmentInfo> for &ChaosRenderAttachment {
    fn into(self) -> RenderingAttachmentInfo {
        let mut vulkan_clear_value = None;
        let load_op = match &self.load_op {
            ChaosRenderAttachmentLoadOp::Load => AttachmentLoadOp::Load,
            ChaosRenderAttachmentLoadOp::Clear(clear_value) => {
                let cv = match *clear_value {
                    ChaosClearValue::Float(f) => ClearValue::Float(f),
                    ChaosClearValue::Int(i) => ClearValue::Int(i),
                    ChaosClearValue::Uint(u) => ClearValue::Uint(u),
                    ChaosClearValue::Depth(d) => ClearValue::Depth(d),
                    ChaosClearValue::Stencil(s) => ClearValue::Stencil(s),
                    ChaosClearValue::DepthStencil(d, s) => ClearValue::DepthStencil((d, s)),
                };
                vulkan_clear_value = Some(cv);
                AttachmentLoadOp::Clear
            }
            ChaosRenderAttachmentLoadOp::DontCare => AttachmentLoadOp::DontCare,
        };

        let store_op = if self.store {
            AttachmentStoreOp::Store
        } else {
            AttachmentStoreOp::DontCare
        };

        let mut info = RenderingAttachmentInfo::image_view(self.texture.image_view());
        info.load_op = load_op;
        info.store_op = store_op;
        info.image_layout = self.layout;

        if let Some(clear_value) = vulkan_clear_value {
            info.clear_value = Some(clear_value);
        }
        return info;
    }
}

impl ChaosCommandBuffer {
    /// Begins a one-time-submit primary buffer.
    pub(crate) fn new(
        buffer_allocator: Arc<StandardCommandBufferAllocator>,
        family_index: u32,
    ) -> Result<Self, ChaosRenderingError> {
        let level = CommandBufferLevel::Primary;
        let begin_info = CommandBufferBeginInfo {
            usage: CommandBufferUsage::OneTimeSubmit,
            ..Default::default()
        };
        let buffer = RecordingCommandBuffer::new(buffer_allocator, family_index, level, begin_info);

        match buffer {
            Ok(b) => Ok(Self {
                cmd_buffer: b,
                inside_rendering: false,
                keep_alive: vec![],
            }),
            Err(e) => Err(ChaosRenderingError::ChaosCommandBufferCreationError(
                format!("{}", e),
            )),
        }
    }

    //  Ends recording. It takes self by value, because ending a recording consumes it. It returns the executable buffer plus the keep-alive list as one unit.
    pub(crate) fn finish(self) -> Result<FinishedFrame, ChaosRenderingError> {
        debug_assert!(!self.inside_rendering);
        let finished_command_buffer = unsafe {
            self.cmd_buffer
                .end()
                .map_err(|e| ChaosRenderingError::ChaosCommandBufferEndError(e.to_string()))
        }?;

        Ok(FinishedFrame {
            cmd_buffer: Arc::new(finished_command_buffer),
            keep_alive: self.keep_alive,
        })
    }

    /// Opens a dynamic rendering scope on the node's output attachments.
    /// It's a good place to also set the viewport and scissor from the render size, since the graph knows it.
    /// It sets the inside_rendering flag.
    pub(crate) fn begin_rendering(
        &mut self,
        color_attachments: &[Option<Arc<ChaosRenderAttachment>>],
        depth_attachment: Option<Arc<ChaosRenderAttachment>>,
        stencil_attachment: Option<Arc<ChaosRenderAttachment>>,
        extents: Option<ChaosRenderExtent>,
    ) -> Result<&mut Self, ChaosRenderingError> {
        let render_area = match &extents {
            Some(extent) => extent.size,
            None => {
                // Size the render area from the first attachment we find,
                // preferring depth, then stencil, then the first color attachment.
                let attachment = depth_attachment
                    .as_ref()
                    .or(stencil_attachment.as_ref())
                    .or_else(|| color_attachments.iter().find_map(|a| a.as_ref()))
                    .ok_or_else(|| {
                        ChaosRenderingError::ChaosCommandBufferBeginRenderingError(
                            "No attachments provided to begin_rendering".to_string(),
                        )
                    })?;
                let texture_dims = attachment.texture.description().dimensions;
                [texture_dims[0], texture_dims[1]]
            }
        };

        let render_offset = if let Some(extent) = &extents {
            extent.offset.unwrap_or([0, 0])
        } else {
            [0, 0]
        };

        let rendering_info = RenderingInfo {
            color_attachments: color_attachments
                .iter()
                .map(|attachment| attachment.clone().map(|a| a.as_ref().into()))
                .collect(),
            depth_attachment: depth_attachment.clone().map(|a| a.as_ref().into()),
            stencil_attachment: stencil_attachment.clone().map(|a| a.as_ref().into()),
            render_area_extent: render_area,
            render_area_offset: render_offset,
            // Raw command buffers don't auto-fill this (the auto builder did);
            // Vulkan requires >= 1 when view_mask is 0.
            layer_count: 1,
            ..Default::default()
        };
        unsafe {
            self.cmd_buffer
                .begin_rendering(&rendering_info)
                .map_err(|e| {
                    ChaosRenderingError::ChaosCommandBufferBeginRenderingError(e.to_string())
                })?;
        }

        let attachments: Vec<Arc<ChaosTexture>> = color_attachments
            .iter()
            .filter_map(|attachment| attachment.as_ref().map(|a| a.texture.clone()))
            .collect();

        self.keep_all(attachments.as_slice());
        if let Some(depth) = depth_attachment {
            self.keep(depth.texture.clone());
        }
        if let Some(stencil) = stencil_attachment {
            self.keep(stencil.texture.clone());
        }

        self.inside_rendering = true;
        Ok(self)
    }

    /// Closes the scope and clears the flag.
    pub(crate) fn end_rendering(&mut self) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer.end_rendering().map_err(|e| {
                ChaosRenderingError::ChaosCommandBufferEndRenderingError(e.to_string())
            })?;
        }
        self.inside_rendering = false;
        Ok(self)
    }

    /// Your add_resource_barrier and add_buffer_barrier call this. You could instead offer image_barrier and buffer_barrier and move the barrier construction in here.
    pub(crate) fn pipeline_barrier(
        &mut self,
        dependency_info: &DependencyInfo,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .pipeline_barrier(dependency_info)
                .map_err(|e| ChaosRenderingError::PipelineBarrierError(e.to_string()))?;
        }

        Ok(self)
    }

    /// For resources the graph touches without binding them, such as attachment images in RenderingInfo and barrier targets. These need to outlive the frame too.
    pub(crate) fn keep<T: Any + Send + Sync>(&mut self, obj: Arc<T>) {
        self.keep_alive.push(obj);
    }

    pub(crate) fn keep_all<T: Any + Send + Sync>(&mut self, objs: &[Arc<T>]) {
        self.keep_alive.extend(
            objs.iter()
                .map(|s| -> Arc<dyn Any + Send + Sync> { s.clone() }),
        );
    }

    pub(crate) fn keep_buffer(&mut self, sub_buffer: &Subbuffer<[u8]>) {
        self.keep_alive.push(sub_buffer.buffer().clone());
    }

    pub(crate) fn keep_all_buffers(&mut self, buffers: &[Subbuffer<[u8]>]) {
        self.keep_alive.extend(
            buffers
                .iter()
                .map(|b| -> Arc<dyn Any + Send + Sync> { b.buffer().clone() }),
        );
    }

    pub fn bind_pipeline_graphics(
        &mut self,
        pipeline: Arc<GraphicsPipeline>,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .bind_pipeline_graphics(&pipeline)
                .map_err(|e| ChaosRenderingError::BindGraphicPipelineError(e.to_string()))?;
        }
        self.keep(pipeline);
        Ok(self)
    }

    /// For QueueKind::Compute nodes.
    pub fn bind_pipeline_compute(
        &mut self,
        pipeline: Arc<ComputePipeline>,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .bind_pipeline_compute(&pipeline)
                .map_err(|e| ChaosRenderingError::BindComputePipelineError(e.to_string()))?;
        }
        self.keep(pipeline);
        Ok(self)
    }

    ///  Used in effect.rs.
    pub fn bind_descriptor_sets(
        &mut self,
        pipeline_bind_point: PipelineBindPoint,
        layout: &PipelineLayout,
        first_set: u32,
        sets: &[Arc<DescriptorSet>],
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            let raw_sets: Vec<&RawDescriptorSet> = sets.iter().map(|s| s.as_raw()).collect();
            self.cmd_buffer
                .bind_descriptor_sets(pipeline_bind_point, layout, first_set, &raw_sets, &[])
                .map_err(|e| ChaosRenderingError::BindDescriptorSetError(e.to_string()))?;
        }

        self.keep_all(sets);
        Ok(self)
    }

    pub fn bind_descriptor_set(
        &mut self,
        pipeline_bind_point: PipelineBindPoint,
        layout: &PipelineLayout,
        first_set: u32,
        set: Arc<DescriptorSet>,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .bind_descriptor_sets(pipeline_bind_point, layout, first_set, &[set.as_raw()], &[])
                .map_err(|e| ChaosRenderingError::BindDescriptorSetError(e.to_string()))?;
        }

        self.keep(set);
        Ok(self)
    }

    pub fn bind_descriptor_set_with_offset(
        &mut self,
        pipeline_bind_point: PipelineBindPoint,
        layout: &PipelineLayout,
        first_set: u32,
        set: Arc<DescriptorSet>,
        offset: u32,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .bind_descriptor_sets(
                    pipeline_bind_point,
                    layout,
                    first_set,
                    &[set.as_raw()],
                    &[offset],
                )
                .map_err(|e| ChaosRenderingError::BindDescriptorSetError(e.to_string()))?;
        }

        self.keep(set);
        Ok(self)
    }

    pub fn bind_descriptor_sets_with_offsets(
        &mut self,
        pipeline_bind_point: PipelineBindPoint,
        layout: &PipelineLayout,
        first_set: u32,
        sets: &[Arc<DescriptorSet>],
        offsets: &[u32],
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            let raw_sets: Vec<&RawDescriptorSet> = sets.iter().map(|s| s.as_raw()).collect();
            self.cmd_buffer
                .bind_descriptor_sets(pipeline_bind_point, layout, first_set, &raw_sets, offsets)
                .map_err(|e| ChaosRenderingError::BindDescriptorSetError(e.to_string()))?;
        }
        self.keep_all(sets);
        Ok(self)
    }

    /// Used in effect.rs.
    pub fn push_constants<T: BufferContents>(
        &mut self,
        layout: Arc<PipelineLayout>,
        offset: u32,
        data: T,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .push_constants(&layout, offset, &data)
                .map_err(|e| ChaosRenderingError::PushConstantError(e.to_string()))?;
        }

        Ok(self)
    }

    pub fn bind_vertex_buffers(
        &mut self,
        first_binding: u32,
        buffers: &[Subbuffer<[u8]>],
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .bind_vertex_buffers(first_binding, buffers)
                .map_err(|e| ChaosRenderingError::BindVertexBuffers(e.to_string()))?;
        }
        self.keep_all_buffers(buffers);
        Ok(self)
    }

    pub fn bind_index_buffer(
        &mut self,
        buffer: &IndexBuffer,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .bind_index_buffer(buffer)
                .map_err(|e| ChaosRenderingError::BindIndexBuffers(e.to_string()))?;
        }

        self.keep_buffer(buffer.as_bytes());
        Ok(self)
    }

    /// Can debug_assert!(inside_rendering)
    pub fn draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .draw(vertex_count, instance_count, first_vertex, first_instance)
                .map_err(|e| ChaosRenderingError::ChaosCommandBufferDrawError(e.to_string()))?;
        }

        Ok(self)
    }

    pub fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .draw_indexed(
                    index_count,
                    instance_count,
                    first_index,
                    vertex_offset,
                    first_instance,
                )
                .map_err(|e| {
                    ChaosRenderingError::ChaosCommandBufferDrawIndexedError(e.to_string())
                })?;
        }

        Ok(self)
    }

    pub fn draw_indexed_indirect(
        &mut self,
        indirect_buffer: &Subbuffer<[DrawIndexedIndirectCommand]>,
        draw_count: u32,
        stride: u32,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .draw_indexed_indirect(indirect_buffer, draw_count, stride)
                .map_err(|e| {
                    ChaosRenderingError::ChaosCommandBufferDrawIndexedIndirectError(e.to_string())
                })?;
        }
        self.keep_buffer(indirect_buffer.as_bytes());
        Ok(self)
    }

    /// Can debug_assert!(!inside_rendering).
    pub fn dispatch(
        &mut self,
        group_x_count: u32,
        group_y_count: u32,
        group_z_count: u32,
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .dispatch([group_x_count, group_y_count, group_z_count])
                .map_err(|e| ChaosRenderingError::ChaosCommandBufferDispatchError(e.to_string()))?;
        }
        Ok(self)
    }

    /// Only needed if a node renders a sub-region. Otherwise the graph handles them in begin_rendering.
    pub fn set_viewport(&mut self, viewport: Viewport) -> Result<&mut Self, ChaosRenderingError> {
        let viewports = [viewport];
        self.set_viewports(0, &viewports)
    }

    pub fn set_viewports(
        &mut self,
        first_viewport: u32,
        viewports: &[Viewport],
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .set_viewport(first_viewport, viewports)
                .map_err(|e| ChaosRenderingError::ViewportError(e.to_string()))?;
        }
        Ok(self)
    }

    pub fn set_scissor(&mut self, scissor: Scissor) -> Result<&mut Self, ChaosRenderingError> {
        let scissors = [scissor];
        self.set_scissors(0, &scissors)
    }

    pub fn set_scissors(
        &mut self,
        first_scissor: u32,
        scissors: &[Scissor],
    ) -> Result<&mut Self, ChaosRenderingError> {
        unsafe {
            self.cmd_buffer
                .set_scissor(first_scissor, scissors)
                .map_err(|e| ChaosRenderingError::ScissorError(e.to_string()))?;
        }
        Ok(self)
    }
}
