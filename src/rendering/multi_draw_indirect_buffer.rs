use std::sync::Arc;

use vulkano::buffer::{BufferContents, Subbuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, DrawIndexedIndirectCommand, PrimaryAutoCommandBuffer,
};

use crate::rendering::buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage};
use crate::rendering::renderer::ChaosRenderContext;

/// Where one mesh lives inside the shared vertex + index buffers.
#[derive(Clone, Copy, Debug)]
pub struct ChaosMultiDrawIndirectBufferSpan {
    pub mesh_id: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub vertex_offset: u32,
}

/// Growable vertex + index + indirect-command buffer trio for
/// `vkCmdDrawIndexedIndirect`.
///
/// - Keeps CPU-side vectors of vertices, indices, and indirect commands.
/// - On `flush()` reuploads the GPU buffers if anything changed.
/// - Each `add` returns a `mesh_id` used as `first_instance`; the vertex shader
///   reads `gl_BaseInstance` to index per-mesh data (e.g. a model matrix SSBO).
pub struct ChaosMultiDrawIndirectBuffer<V: BufferContents + Copy> {
    pub name: String,

    vertices: Vec<V>,
    indices: Vec<u32>,
    commands: Vec<DrawIndexedIndirectCommand>,
    spans: Vec<ChaosMultiDrawIndirectBufferSpan>,

    vertex_buffer: ChaosBuffer,
    index_buffer: ChaosBuffer,
    indirect_buffer: ChaosBuffer,

    dirty: bool,
}

impl<V: BufferContents + Copy> ChaosMultiDrawIndirectBuffer<V> {
    pub fn new(name: impl Into<String>, render_context: Arc<ChaosRenderContext>) -> Self {
        let name = name.into();
        Self {
            vertex_buffer: ChaosBuffer::new(
                format!("{}-vertex", name),
                ChaosBufferUsage::VertexBuffer,
                ChaosBufferMemoryType::PreferDevice,
                render_context.clone(),
            ),
            index_buffer: ChaosBuffer::new(
                format!("{}-index", name),
                ChaosBufferUsage::IndexBuffer,
                ChaosBufferMemoryType::PreferDevice,
                render_context.clone(),
            ),
            indirect_buffer: ChaosBuffer::new(
                format!("{}-indirect", name),
                ChaosBufferUsage::IndirectBuffer,
                ChaosBufferMemoryType::PreferHost,
                render_context.clone(),
            ),
            name,
            vertices: Vec::new(),
            indices: Vec::new(),
            commands: Vec::new(),
            spans: Vec::new(),
            dirty: false,
        }
    }

    /// Append a mesh. Returns its span; also indexable by `mesh_id`.
    pub fn add<T, I>(&mut self, vertices: T, indices: I) -> ChaosMultiDrawIndirectBufferSpan
    where
        T: IntoIterator<Item = V>,
        I: IntoIterator<Item = u32>,
    {
        let mesh_id = self.spans.len() as u32;
        let vertex_offset = self.vertices.len() as u32;
        let first_index = self.indices.len() as u32;

        self.vertices.extend(vertices);
        let before = self.indices.len();

        self.indices.extend(indices);
        let index_count = (self.indices.len() - before) as u32;

        let span = ChaosMultiDrawIndirectBufferSpan {
            mesh_id,
            first_index,
            index_count,
            vertex_offset,
        };
        self.spans.push(span);

        self.commands.push(DrawIndexedIndirectCommand {
            index_count,
            instance_count: 1,
            first_index,
            vertex_offset,
            first_instance: mesh_id,
        });

        self.dirty = true;
        span
    }

    /// Drop all meshes. GPU buffers are reallocated on the next `flush`.
    pub fn clear(&mut self) {
        if self.spans.is_empty() {
            return;
        }
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.spans.clear();
        self.dirty = true;
    }

    pub fn spans(&self) -> &[ChaosMultiDrawIndirectBufferSpan] {
        &self.spans
    }

    pub fn draw_count(&self) -> u32 {
        self.commands.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Reupload changed data. Call before recording draws for this frame.
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if self.vertices.is_empty() || self.indices.is_empty() || self.commands.is_empty() {
            self.dirty = false;
            return Ok(());
        }
        self.vertex_buffer
            .set_data_from_vec(self.vertices.clone())?;
        self.index_buffer.set_data_from_vec(self.indices.clone())?;
        self.indirect_buffer
            .set_data_from_vec(self.commands.clone())?;
        self.dirty = false;
        Ok(())
    }

    pub fn vertex_subbuffer(&self) -> Option<Subbuffer<[V]>> {
        self.vertex_buffer
            .buffer()
            .map(|arc| (*arc).clone().reinterpret::<[V]>())
    }

    pub fn index_subbuffer(&self) -> Option<Subbuffer<[u32]>> {
        self.index_buffer
            .buffer()
            .map(|arc| (*arc).clone().reinterpret::<[u32]>())
    }

    pub fn indirect_subbuffer(&self) -> Option<Subbuffer<[DrawIndexedIndirectCommand]>> {
        self.indirect_buffer
            .buffer()
            .map(|arc| (*arc).clone().reinterpret::<[DrawIndexedIndirectCommand]>())
    }

    pub fn draw(&self, command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        if self.commands.is_empty() {
            return;
        }

        self.index_buffer
            .bind_as_index_buffer(command_buffer)
            .map_err(|e| format!("Failed to bind index buffer for {}: {:?}", self.name, e))
            .unwrap();
        self.vertex_buffer
            .bind_as_vertex_buffer(0, command_buffer)
            .map_err(|e| format!("Failed to bind vertex buffer for {}: {:?}", self.name, e))
            .unwrap();

        let indirect_subbuffer = self.indirect_subbuffer().ok_or_else(|| {
            format!(
                "Failed to get indirect buffer for {}. Did you call flush()?",
                self.name
            )
        });

        unsafe {
            command_buffer
                .draw_indexed_indirect(indirect_subbuffer.unwrap())
                .map_err(|e| {
                    format!(
                        "Failed to record draw_indexed_indirect for {}: {:?}",
                        self.name, e
                    )
                })
                .unwrap();
        }
    }
}
