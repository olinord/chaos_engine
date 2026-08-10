use std::fmt::Display;
use std::sync::Arc;

use vulkano::ValidationError;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};

use crate::rendering::renderer::ChaosRenderContext;

#[derive(Debug, Clone)]
pub enum ChaosBufferUsage {
    TransferSrc,
    TransferDst,
    UniformTexelBuffer,
    StorageTexelBuffer,
    UniformBuffer,
    StorageBuffer,
    IndexBuffer,
    VertexBuffer,
    IndirectBuffer,
    DeviceAddress,
    AccelerationStructureBuildInputReadOnly,
    AccelerationStructureStorage,
    ShaderBindingTable,
    Invalid,
}

#[derive(Debug, Clone)]
pub enum BufferError {
    ValidationError(Box<ValidationError>),
    BufferNotBound,
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

    pub fn set_data_from_vec<T: BufferContents>(&mut self, data: Vec<T>) -> Result<(), String> {
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
                Err(format!("{:?}", error))
            }
        }
    }

    pub fn set_data<T: BufferContents>(&mut self, data: T) -> Result<(), String> {
        self.set_data_from_vec(vec![data])
    }

    pub fn buffer(&self) -> Option<Arc<Subbuffer<[u8]>>> {
        self.buffer.clone()
    }

    pub fn length(&self) -> u32 {
        self.length as u32
    }

    pub fn bind_as_index_buffer(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), BufferError> {
        if let Some(buffer) = &self.buffer {
            let buffer = buffer.as_ref().clone().reinterpret::<[u32]>();
            command_buffer
                .bind_index_buffer(buffer)
                .map_err(|e| BufferError::ValidationError(e))?;
            Ok(())
        } else {
            Err(BufferError::BufferNotBound)
        }
    }

    pub fn bind_as_vertex_buffer(
        &self,
        index: u32,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), BufferError> {
        if let Some(buffer) = &self.buffer {
            let buffer = buffer.as_ref().clone();
            command_buffer
                .bind_vertex_buffers(index, buffer)
                .map_err(|e| BufferError::ValidationError(e))?;
            Ok(())
        } else {
            Err(BufferError::BufferNotBound)
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
            ChaosBufferUsage::IndexBuffer => BufferUsage::INDEX_BUFFER,
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
                panic!("Trying to convert Invalid ChaosBufferUsage to Vulkano BufferUsage")
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

impl Display for BufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BufferError::ValidationError(e) => write!(f, "Buffer validation error: {:?}", e),
            BufferError::BufferNotBound => write!(f, "Buffer not bound"),
        }
    }
}
