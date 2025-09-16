use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};

#[derive(Debug, Clone)]
pub enum CEBufferUsage {
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
}

#[derive(Debug, Clone)]
pub enum CEBufferMemoryType {
    PreferDevice,
    PreferHost,
    HostSequentialWrite,
    HostRandomAccess,
}

#[derive(Debug, Clone)]
pub struct CEBuffer<T> {
    pub buffer: Arc<Subbuffer<[T]>>,
}

#[derive(Debug, Clone)]
pub struct CEBufferBuilder {
    name: String,
    allocator: Option<Arc<StandardMemoryAllocator>>,
    usage: Option<BufferUsage>,
    memory_type_filter: MemoryTypeFilter,
}

impl CEBufferBuilder {
    pub fn new(name: String) -> CEBufferBuilder {
        return CEBufferBuilder {
            name,
            allocator: None,
            usage: None,
            memory_type_filter: MemoryTypeFilter::empty(),
        };
    }

    pub fn with_allocator(mut self, allocator: Arc<StandardMemoryAllocator>) -> CEBufferBuilder {
        if self.allocator.is_some() {
            log::warn!(
                "Memory allocator already specified for buffer '{}'",
                self.name
            );
        }
        self.allocator = Some(allocator);
        self
    }

    pub fn with_usage(mut self, usage: CEBufferUsage) -> CEBufferBuilder {
        if self.usage.is_some() {
            self.usage = Some(self.usage.unwrap() | usage.into());
        } else {
            self.usage = Some(usage.into());
        }

        self
    }

    pub fn with_memory_type(mut self, memory_type: CEBufferMemoryType) -> CEBufferBuilder {
        self.memory_type_filter = self.memory_type_filter | memory_type.into();
        self
    }

    pub fn build<T>(self, data: Vec<T>) -> Result<CEBuffer<T>, &'static str>
    where
        T: BufferContents,
    {
        if self.allocator.is_none() {
            return Err("No allocator specified");
        }
        if self.usage.is_none() {
            return Err("No usage specified");
        }

        let buffer = Buffer::from_iter(
            self.allocator.unwrap(),
            BufferCreateInfo {
                usage: self.usage.unwrap(),
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: self.memory_type_filter,
                ..Default::default()
            },
            data,
        )
        .expect("failed to create buffer");
        return Ok(CEBuffer {
            buffer: Arc::new(buffer),
        });
    }
}

impl Into<BufferUsage> for CEBufferUsage {
    fn into(self) -> BufferUsage {
        match self {
            CEBufferUsage::TransferSrc => BufferUsage::TRANSFER_SRC,
            CEBufferUsage::TransferDst => BufferUsage::TRANSFER_DST,
            CEBufferUsage::UniformTexelBuffer => BufferUsage::UNIFORM_TEXEL_BUFFER,
            CEBufferUsage::StorageTexelBuffer => BufferUsage::STORAGE_TEXEL_BUFFER,
            CEBufferUsage::UniformBuffer => BufferUsage::UNIFORM_BUFFER,
            CEBufferUsage::StorageBuffer => BufferUsage::STORAGE_BUFFER,
            CEBufferUsage::IndexBuffer => BufferUsage::INDEX_BUFFER,
            CEBufferUsage::VertexBuffer => BufferUsage::VERTEX_BUFFER,
            CEBufferUsage::IndirectBuffer => BufferUsage::INDIRECT_BUFFER,
            CEBufferUsage::DeviceAddress => BufferUsage::SHADER_DEVICE_ADDRESS,
            CEBufferUsage::AccelerationStructureBuildInputReadOnly => {
                BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY
            }
            CEBufferUsage::AccelerationStructureStorage => {
                BufferUsage::ACCELERATION_STRUCTURE_STORAGE
            }
            CEBufferUsage::ShaderBindingTable => BufferUsage::SHADER_BINDING_TABLE,
        }
    }
}

impl Into<MemoryTypeFilter> for CEBufferMemoryType {
    fn into(self) -> MemoryTypeFilter {
        match self {
            CEBufferMemoryType::PreferDevice => MemoryTypeFilter::PREFER_DEVICE,
            CEBufferMemoryType::PreferHost => MemoryTypeFilter::PREFER_HOST,
            CEBufferMemoryType::HostSequentialWrite => MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            CEBufferMemoryType::HostRandomAccess => MemoryTypeFilter::HOST_RANDOM_ACCESS,
        }
    }
}
