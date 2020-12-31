use core::mem;
use std::mem::ManuallyDrop;

use gfx_hal::{
    adapter::PhysicalDevice,
    Backend,
    buffer,
    device::Device,
    memory::{Properties, Segment},
};
use gfx_hal::command::CommandBuffer;

pub trait BufferData {
    fn layout() -> Vec<u32>;
}

pub struct Buffer<B: Backend, T: BufferData> {
    data: Vec<T>,
    initialized: bool,
    // gfx things
    buffer: Option<ManuallyDrop<B::Buffer>>,
    #[allow(unused)]
    buffer_memory: Option<ManuallyDrop<B::Memory>>,
    vertex_count: usize,
}

impl<B: Backend, T: BufferData> Buffer<B, T> {
    //
    pub fn new(data_list: Vec<T>) -> Buffer<B, T> {
        let c = data_list.len();
        return Buffer{
            data: data_list,
            initialized: false,
            buffer: Option::None,
            buffer_memory: Option::None,
            vertex_count: c
        }
    }

    pub fn initialize(&mut self, device: &B::Device, physical_device: &B::PhysicalDevice) {
        if self.initialized {
            return;
        }

        let buffer_stride = mem::size_of::<T>() as usize;
        let buffer_len = self.vertex_count * buffer_stride;
        // create empty buffer
        let (buffer_memory, buffer) =  unsafe {
            Buffer::<B, T>::make_buffer(device,
                                     physical_device,
                                     buffer_len,
                                     buffer::Usage::VERTEX,
                                     Properties::CPU_VISIBLE)
        };

        // fill it
        unsafe {

            // Mapping the buffer memory gives us a pointer directly to the
            // contents of the buffer, which lets us easily copy data into it.
            //
            // We pass `Segment::ALL` to say that we want to map the *whole*
            // buffer, as opposed to just part of it.
            let mapped_memory = device
                .map_memory(&buffer_memory, Segment::ALL)
                .expect("Failed to map memory");

            // copy the memory to the buffer
            std::ptr::copy_nonoverlapping(self.data.as_ptr() as *const u8, mapped_memory, buffer_len);

            // Flushing the mapped memory ensures that the data we wrote to the
            // memory actually makes it to the graphics device. The copy alone does
            // not guarantee this.
            //
            // Again, we could supply multiple ranges (of multiple buffers even)
            // but instead we just flush `ALL` of our single buffer.
            device
                .flush_mapped_memory_ranges(vec![(&buffer_memory, Segment::ALL)])
                .expect("Out of memory");

            device.unmap_memory(&buffer_memory);
        }
        self.buffer = Some(ManuallyDrop::new(buffer));
        self.buffer_memory = Some(ManuallyDrop::new(buffer_memory));

        self.data.clear(); // no need to store the data anymor
        self.initialized = true;
    }

    pub fn bind_to_cmd_buffer(&self, cmd_buffer: &mut B::CommandBuffer) {
        if !self.initialized {
            println!("Trying to bind un-initialized buffer to command buffer. Doing nothing instead");
            return;
        }
        unsafe {
            if let Some(b) = &self.buffer {
                cmd_buffer.bind_vertex_buffers(
                    0,
                    vec![(&**b, gfx_hal::buffer::SubRange::WHOLE)],
                )
            }
        }
    }

    pub fn is_initialized(&self) -> bool {
        return self.initialized;
    }
    pub fn get_length(&self) -> u32 {
        return self.vertex_count as u32;
    }

    /// Create an empty buffer with the given size and properties.
    ///
    /// Buffers can be used for various things. The `usage` parameter defines
    /// how the buffer should be treated (vertex buffer, index buffer, etc).
    /// The `properties` specify the kind of memory that should be used to
    /// store this buffer (CPU visible, device local, etc).
    /// taken fom https://github.com/mistodon/gfx-hal-tutorials/blob/master/src/bin/part-3-vertex-buffers.rs
    unsafe fn make_buffer(
        device: &B::Device,
        physical_device: &B::PhysicalDevice,
        buffer_len: usize,
        usage: gfx_hal::buffer::Usage,
        properties: gfx_hal::memory::Properties,
    ) -> (B::Memory, B::Buffer) {
        use gfx_hal::MemoryTypeId;

        // This creates a handle to a buffer. The `buffer_len` is in bytes,
        // and the usage states what kind of buffer it is. For this part,
        // we're making a vertex buffer, so you'll see later that we pass
        // `Usage::VERTEX` for this parameter.
        let mut buffer = device
            .create_buffer(buffer_len as u64, usage)
            .expect("Failed to create buffer");

        // The device may have its own requirements for storing a buffer of
        // this certain size and properties. It returns a `Requirements` struct
        // from which we'll use two fields: `size` and `type_mask`.
        //
        // The `size` field should be pretty straightforward - it may differ
        // from `buffer_len` if there are padding/alignment requirements.
        //
        // The `type_mask` is a bitmask representing which memory types are
        // compatible.
        let req = device.get_buffer_requirements(&buffer);

        // This list of `memory_type` corresponds to the list represented by
        // the `type_mask` above. If the nth bit in the mask is `1`, then the
        // nth memory type in this list is supported.
        let memory_types = physical_device.memory_properties().memory_types;

        // We iterate over all the memory types and select the first one that
        // is both supported (e.g. in the `type_mask`), and supports the
        // `properties` we requested. In our case this is `CPU_VISIBLE` as
        // we'll see later.
        let memory_type = memory_types
            .iter()
            .enumerate()
            .find(|(id, mem_type)| {
                let type_supported = req.type_mask & (1_u32 << id) != 0;
                type_supported && mem_type.properties.contains(properties)
            })
            .map(|(id, _ty)| MemoryTypeId(id))
            .expect("No compatible memory type available");

        // Now that we know the size and type of the memory to allocate, we can
        // go ahead and do so.
        let buffer_memory = device
            .allocate_memory(memory_type, req.size)
            .expect("Failed to allocate buffer memory");

        // Now that we have memory to back our buffer, we can bind that buffer
        // handle to the memory. That buffer now has some actual storage
        // associated with it.
        device
            .bind_buffer_memory(&buffer_memory, 0, &mut buffer)
            .expect("Failed to bind buffer memory");

        (buffer_memory, buffer)
    }
}

impl<B: Backend, T: BufferData> Drop for Buffer<B, T> {
    fn drop(&mut self) {
        // self.device.free_memory(ManuallyDrop::into_inner(ptr::read(&self.buffer_memory)));
    }
}