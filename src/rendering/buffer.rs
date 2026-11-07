use std::sync::Arc;

use vulkano::buffer::{
    Buffer, BufferContents, BufferCreateInfo, BufferUsage, IndexBuffer, Subbuffer,
};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::pipeline::graphics::vertex_input::Vertex;

use crate::rendering::command_buffer::ChaosCommandBuffer;
use crate::rendering::context::ChaosRenderContext;
use crate::rendering::errors::RenderError;

#[derive(Debug, Clone)]
pub enum ChaosBufferUsage {
    TransferSrc,
    TransferDst,
    UniformTexelBuffer,
    StorageTexelBuffer,
    UniformBuffer,
    StorageBuffer,
    VertexBuffer,
    IndirectBuffer,
    DeviceAddress,
    AccelerationStructureBuildInputReadOnly,
    AccelerationStructureStorage,
    ShaderBindingTable,
    Invalid,
}

#[derive(Debug, Clone)]
pub enum ChaosBufferMemoryType {
    PreferDevice,
    PreferHost,
    HostSequentialWrite,
    HostRandomAccess,
}

#[derive(Debug, Clone)]
pub struct ChaosBuffer {
    pub name: String,
    buffer: Option<Arc<Subbuffer<[u8]>>>,
    usage: ChaosBufferUsage,
    memory_type_filter: ChaosBufferMemoryType,
    length: usize,
    render_context: Arc<ChaosRenderContext>,
}

#[derive(Debug, Clone)]
pub struct ChaosVertexBuffer<V: Vertex> {
    pub name: String,
    buffer: Option<Arc<Subbuffer<[V]>>>,
    length: usize,
    render_context: Arc<ChaosRenderContext>,
}

#[derive(Debug, Clone)]
pub struct ChaosIndexBuffer {
    pub name: String,
    buffer: Option<Arc<IndexBuffer>>,
    length: usize,
    render_context: Arc<ChaosRenderContext>,
}

impl ChaosBuffer {
    pub fn new(
        name: String,
        usage: ChaosBufferUsage,
        memory_type_filter: ChaosBufferMemoryType,
        render_context: Arc<ChaosRenderContext>,
    ) -> ChaosBuffer {
        ChaosBuffer {
            name,
            buffer: None,
            usage,
            memory_type_filter,
            length: 0,
            render_context,
        }
    }

    pub fn set_data_from_vec<T: BufferContents>(
        &mut self,
        data: Vec<T>,
    ) -> Result<(), RenderError> {
        self.length = data.len();

        let buffer = Buffer::from_iter(
            self.render_context.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::from(self.usage.clone()),
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::from(self.memory_type_filter.clone()),
                ..Default::default()
            },
            data.into_iter(),
        );

        match buffer {
            Ok(buffer) => {
                self.buffer = Some(Arc::new(buffer.into_bytes()));
                Ok(())
            }
            Err(error) => {
                self.buffer = None;
                self.length = 0;
                Err(RenderError::allocation(self.name.clone(), error))
            }
        }
    }

    pub fn set_data<T: BufferContents>(&mut self, data: T) -> Result<(), RenderError> {
        self.set_data_from_vec(vec![data])
    }

    pub fn buffer(&self) -> Option<Arc<Subbuffer<[u8]>>> {
        self.buffer.clone()
    }

    pub fn length(&self) -> u32 {
        self.length as u32
    }

    pub fn bind(
        &self,
        index: u32,
        command_buffer: &mut ChaosCommandBuffer,
    ) -> Result<(), RenderError> {
        if let Some(buffer) = &self.buffer {
            let buffer = buffer.as_ref().clone();
            command_buffer.bind_vertex_buffers(index, &[buffer]);
            Ok(())
        } else {
            Err(RenderError::BufferNotUploaded {
                name: self.name.clone(),
            })
        }
    }
}

impl<V: Vertex> ChaosVertexBuffer<V> {
    pub fn new(name: String, render_context: Arc<ChaosRenderContext>) -> ChaosVertexBuffer<V> {
        ChaosVertexBuffer {
            name,
            buffer: None,
            length: 0,
            render_context,
        }
    }

    pub fn set_data_from_vec(&mut self, data: Vec<V>) -> Result<(), RenderError> {
        self.length = data.len();

        let buffer = Buffer::from_iter(
            self.render_context.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            data.into_iter(),
        );

        match buffer {
            Ok(buffer) => {
                self.buffer = Some(Arc::new(buffer));
                Ok(())
            }
            Err(error) => {
                self.buffer = None;
                self.length = 0;
                Err(RenderError::allocation(self.name.clone(), error))
            }
        }
    }

    pub fn set_data(&mut self, data: V) -> Result<(), RenderError> {
        self.set_data_from_vec(vec![data])
    }

    pub fn bind(&self, command_buffer: &mut ChaosCommandBuffer) -> Result<(), RenderError> {
        if let Some(buffer) = &self.buffer {
            let buffer = buffer.as_ref().clone();
            command_buffer.bind_vertex_buffers(0, &[buffer.into_bytes()]);
            Ok(())
        } else {
            Err(RenderError::BufferNotUploaded {
                name: self.name.clone(),
            })
        }
    }

    pub fn buffer(&self) -> Option<Arc<Subbuffer<[V]>>> {
        self.buffer.clone()
    }

    pub fn length(&self) -> u32 {
        self.length as u32
    }

    pub fn stride(&self) -> u32 {
        V::per_vertex().stride
    }
}

impl ChaosIndexBuffer {
    pub fn new(name: String, render_context: Arc<ChaosRenderContext>) -> ChaosIndexBuffer {
        ChaosIndexBuffer {
            name,
            buffer: None,
            length: 0,
            render_context,
        }
    }

    pub fn set_u8(&mut self, data: Vec<u8>) -> Result<(), RenderError> {
        self.length = data.len();

        let buffer = Buffer::from_iter(
            self.render_context.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            data.into_iter(),
        )
        .map_err(|e| RenderError::allocation(self.name.clone(), e))?;

        self.buffer = Some(Arc::new(IndexBuffer::from(buffer)));
        Ok(())
    }

    pub fn set_u16(&mut self, data: Vec<u16>) -> Result<(), RenderError> {
        self.length = data.len();

        let buffer = Buffer::from_iter(
            self.render_context.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            data.into_iter(),
        )
        .map_err(|e| RenderError::allocation(self.name.clone(), e))?;

        self.buffer = Some(Arc::new(IndexBuffer::from(buffer)));
        Ok(())
    }

    pub fn set_u32(&mut self, data: Vec<u32>) -> Result<(), RenderError> {
        self.length = data.len();

        let buffer = Buffer::from_iter(
            self.render_context.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            data.into_iter(),
        )
        .map_err(|e| RenderError::allocation(self.name.clone(), e))?;

        self.buffer = Some(Arc::new(IndexBuffer::from(buffer)));
        Ok(())
    }

    pub fn bind(&self, command_buffer: &mut ChaosCommandBuffer) -> Result<(), RenderError> {
        if let Some(buffer) = &self.buffer {
            let buffer = buffer.as_ref().clone();
            command_buffer.bind_index_buffer(&buffer);
            Ok(())
        } else {
            Err(RenderError::BufferNotUploaded {
                name: self.name.clone(),
            })
        }
    }
}

impl From<ChaosBufferUsage> for BufferUsage {
    fn from(value: ChaosBufferUsage) -> BufferUsage {
        match value {
            ChaosBufferUsage::TransferSrc => BufferUsage::TRANSFER_SRC,
            ChaosBufferUsage::TransferDst => BufferUsage::TRANSFER_DST,
            ChaosBufferUsage::UniformTexelBuffer => BufferUsage::UNIFORM_TEXEL_BUFFER,
            ChaosBufferUsage::StorageTexelBuffer => BufferUsage::STORAGE_TEXEL_BUFFER,
            ChaosBufferUsage::UniformBuffer => BufferUsage::UNIFORM_BUFFER,
            ChaosBufferUsage::StorageBuffer => BufferUsage::STORAGE_BUFFER,
            ChaosBufferUsage::VertexBuffer => BufferUsage::VERTEX_BUFFER,
            ChaosBufferUsage::IndirectBuffer => BufferUsage::INDIRECT_BUFFER,
            ChaosBufferUsage::DeviceAddress => BufferUsage::SHADER_DEVICE_ADDRESS,
            ChaosBufferUsage::AccelerationStructureBuildInputReadOnly => {
                BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY
            }
            ChaosBufferUsage::AccelerationStructureStorage => {
                BufferUsage::ACCELERATION_STRUCTURE_STORAGE
            }
            ChaosBufferUsage::ShaderBindingTable => BufferUsage::SHADER_BINDING_TABLE,
            ChaosBufferUsage::Invalid => {
                log::error!("Trying to convert Invalid ChaosBufferUsage to Vulkano BufferUsage");
                BufferUsage::empty()
            }
        }
    }
}

impl From<ChaosBufferMemoryType> for MemoryTypeFilter {
    fn from(value: ChaosBufferMemoryType) -> MemoryTypeFilter {
        match value {
            ChaosBufferMemoryType::PreferDevice => {
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
            }
            ChaosBufferMemoryType::PreferHost => {
                MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
            }
            ChaosBufferMemoryType::HostSequentialWrite => MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ChaosBufferMemoryType::HostRandomAccess => MemoryTypeFilter::HOST_RANDOM_ACCESS,
        }
    }
}
