use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};

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
pub enum ChaosBufferMemoryType {
    PreferDevice,
    PreferHost,
    HostSequentialWrite,
    HostRandomAccess,
}

#[derive(Debug, Clone)]
pub struct ChaosBuffer<T: BufferContents + Send + Sync> {
    pub name: String,
    pub buffer: Option<Arc<Subbuffer<[T]>>>,
    pub usage: ChaosBufferUsage,
    memory_type_filter: ChaosBufferMemoryType,
    pub data: Vec<T>,
}

impl<T> Default for ChaosBuffer<T>
where
    T: BufferContents + Send + Sync,
{
    fn default() -> Self {
        ChaosBuffer {
            name: String::new(),
            buffer: None,
            usage: ChaosBufferUsage::Invalid,
            memory_type_filter: ChaosBufferMemoryType::PreferDevice,
            data: Vec::new(),
        }
    }
}

impl<T> ChaosBuffer<T>
where
    T: BufferContents + Send + Sync,
{
    pub fn new(
        name: String,
        data: Vec<T>,
        usage: ChaosBufferUsage,
        memory_type_filter: ChaosBufferMemoryType,
    ) -> ChaosBuffer<T> {
        return ChaosBuffer::<T> {
            name,
            buffer: None,
            usage,
            memory_type_filter,
            data,
        };
    }

    pub fn initialize(&mut self, allocator: Arc<StandardMemoryAllocator>) -> Result<(), String> {
        let v: Vec<T> = self.data.drain(..).into_iter().collect();
        let buffer = Buffer::from_iter(
            allocator,
            BufferCreateInfo {
                usage: self.usage.clone().into(),
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: self.memory_type_filter.clone().into(),
                ..Default::default()
            },
            v,
        );

        match buffer {
            Ok(buffer) => {
                self.buffer = Some(Arc::new(buffer));
                Ok(())
            }
            Err(error) => Err(format!("{:?}", error)),
        }
    }
}

impl Into<BufferUsage> for ChaosBufferUsage {
    fn into(self) -> BufferUsage {
        match self {
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

impl Into<MemoryTypeFilter> for ChaosBufferMemoryType {
    fn into(self) -> MemoryTypeFilter {
        match self {
            ChaosBufferMemoryType::PreferDevice => MemoryTypeFilter::PREFER_DEVICE,
            ChaosBufferMemoryType::PreferHost => MemoryTypeFilter::PREFER_HOST,
            ChaosBufferMemoryType::HostSequentialWrite => MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ChaosBufferMemoryType::HostRandomAccess => MemoryTypeFilter::HOST_RANDOM_ACCESS,
        }
    }
}
