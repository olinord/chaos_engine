use core::{iter, mem, ptr};
use std::any::Any;
use std::mem::ManuallyDrop;

use gfx_hal::{
    adapter::PhysicalDevice,
    Backend,
    buffer,
    device::Device,
    memory::{Properties, Segment},
};
use gfx_hal::command::CommandBuffer;

pub struct Buffer<'a, B: Backend> {
    buffer: ManuallyDrop<B::Buffer>,
    buffer_memory: ManuallyDrop<B::Memory>,
    device: &'a B::Device,
}

impl<'a, B: Backend> Buffer<'a, B> {
    pub fn vertex_buffer<T: Any>(data_list: &[T], device: &'a B::Device, physical_device: &B::PhysicalDevice) -> Buffer<'a, B> {
        let memory_types = physical_device.memory_properties().memory_types;
        let limits = physical_device.limits();

        // Buffer allocations
        log::info!("Memory types: {:?}", memory_types);
        let non_coherent_alignment = limits.non_coherent_atom_size as u64;

        let buffer_stride = mem::size_of::<T>() as u64;
        let buffer_len = data_list.len() as u64 * buffer_stride;
        assert_ne!(buffer_len, 0);
        let padded_buffer_len = ((buffer_len + non_coherent_alignment - 1)
            / non_coherent_alignment)
            * non_coherent_alignment;

        let mut buffer = ManuallyDrop::new(
            unsafe { device.create_buffer(padded_buffer_len, buffer::Usage::VERTEX) }.unwrap(),
        );

        let buffer_req = unsafe { device.get_buffer_requirements(&buffer) };

        let upload_type = memory_types
            .iter()
            .enumerate()
            .position(|(id, mem_type)| {
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                buffer_req.type_mask & (1 << id) != 0
                    && mem_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .unwrap()
            .into();

        let buffer_memory = unsafe {
            let memory = device
                .allocate_memory(upload_type, buffer_req.size)
                .unwrap();
            device
                .bind_buffer_memory(&memory, 0, &mut buffer)
                .unwrap();
            let mapping = device.map_memory(&memory, Segment::ALL).unwrap();
            ptr::copy_nonoverlapping(data_list.as_ptr() as *const u8, mapping, buffer_len as usize);
            device
                .flush_mapped_memory_ranges(iter::once((&memory, Segment::ALL)))
                .unwrap();
            device.unmap_memory(&memory);
            ManuallyDrop::new(memory)
        };
        Buffer { buffer: buffer, buffer_memory: buffer_memory, device }
    }

    pub fn bind_to_cmd_buffer(&mut self, cmd_buffer: &mut B::CommandBuffer) {
        unsafe {
            cmd_buffer.bind_vertex_buffers(
                0,
                iter::once((&*self.buffer, buffer::SubRange::WHOLE)),
            );
        }
    }
}

impl<'a, B: Backend> Drop for Buffer<'a, B> {
    fn drop(&mut self) {
        // self.buffer
    }
}