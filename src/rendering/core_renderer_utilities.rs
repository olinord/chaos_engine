use std::mem::ManuallyDrop;

use gfx_hal::{
    adapter::{Adapter, Gpu, PhysicalDevice},
    Backend,
    command::Level,
    device::Device,
    Features,
    format::{ChannelType, Format},
    Instance,
    pool::{CommandPool, CommandPoolCreateFlags},
    queue::{QueueFamily, QueueFamilyId, QueueGroup},
    window::Surface,
};
use gfx_hal::image::Layout;
use gfx_hal::pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDesc};
use gfx_hal::window::{Extent2D, PresentationSurface, SwapchainConfig};

// use std::io::Cursor;

pub fn create_command_pool<B: Backend>(device: &B::Device, queue_group_family: QueueFamilyId) -> B::CommandPool {
    unsafe {
        device.
            create_command_pool(queue_group_family, CommandPoolCreateFlags::empty()).
            expect("Out of memory while creating command pool")
    }
}

pub fn create_command_buffer<B: Backend>(command_pool: &mut B::CommandPool) -> B::CommandBuffer {
    unsafe {
        command_pool.allocate_one(Level::Primary)
    }
}

pub fn extract_device_and_queue_group<B: Backend>(adapter: &Adapter<B>, surface: &B::Surface) -> Result<(B::Device, QueueGroup<B>), &'static str> {
    // Open A Device and take out a QueueGroup
    let queue_family = adapter
        .queue_families
        .iter()
        .find(|qf| qf.queue_type().supports_graphics() && surface.supports_queue_family(qf))
        .ok_or("Couldn't find a QueueFamily with graphics!")?;

    let mut gpu: Gpu<B> = unsafe {
        adapter
            .physical_device
            .open(&[(&queue_family, &[1.0; 1])], Features::empty())
            .map_err(|_| "Couldn't open the PhysicalDevice!")?
    };
    let device = gpu.device;
    let queue_group = gpu.queue_groups.pop().ok_or("Failed to get a QueueGrouop!")?;

    if queue_group.queues.len() > 0 {
        Ok((device, queue_group))
    } else {
        Err("The QueueGroup did not have any CommandQueues available!")
    }
}

pub fn create_swapchain<B: Backend>(extent: Extent2D, adapter: &Adapter<B>, surface: &mut B::Surface, device: &B::Device) {
    let capabilities = surface.capabilities(&adapter.physical_device);

    let formats = surface.supported_formats(&adapter.physical_device);
    println!("formats: {:?}", formats);
    let format = formats.map_or(Format::Rgba8Srgb, |formats| {
        formats
            .iter()
            .find(|format| format.base_format().1 == ChannelType::Srgb)
            .map(|format| *format)
            .unwrap_or(formats[0])
    });

    let swapchain_config = SwapchainConfig::from_caps(&capabilities, format, extent);

    unsafe {
        surface.configure_swapchain(&device, swapchain_config.clone())
            .expect("Failed to configure swapchain");
    }
}

pub fn extract_adapter<B: Backend>(instance: &B::Instance, surface: &B::Surface) -> Adapter<B> {
    let adapter = instance
        .enumerate_adapters()
        .into_iter()
        .find(|a| {
            a.queue_families
                .iter()
                .any(|qf| qf.queue_type().supports_graphics() && surface.supports_queue_family(qf))
        })
        .ok_or("Couldn't find a graphical Adapter!").unwrap();

    log::info!("Using adapter {}", adapter.info.name);
    return adapter;
}

pub fn create_render_pass<B: Backend>(device: &B::Device) -> ManuallyDrop<B::RenderPass> {
    let render_pass = {
        let color_attachment = Attachment {
            format: Some(Format::Rgba8Srgb),
            samples: 1,
            ops: AttachmentOps {
                load: AttachmentLoadOp::Clear,
                store: AttachmentStoreOp::Store,
            },
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::Undefined..Layout::Present,
        };
        let subpass = SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        };
        ManuallyDrop::new(
            unsafe { device.create_render_pass(&[color_attachment], &[subpass], &[]) }
                .expect("Can't create render pass"),
        )
    };
    render_pass
}

/*
pub fn create_pipeline<B: Backend, T: Any>(vs_path: &'static str, ps_path: &'static str, ) -> ManuallyDrop<B::GraphicsPipeline>{
    let pipeline_layout = ManuallyDrop::new(
        unsafe {
            device.create_pipeline_layout(
                iter::once(&*set_layout),
                &[(ShaderStageFlags::VERTEX, 0..8)],
            )
        }
            .expect("Can't create pipeline layout"),
    );
    let pipeline = {
        let vs_module = {
            let spirv =
                auxil::read_spirv(Cursor::new(&include_bytes!(vs_path)[..]))
                    .unwrap();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };
        let fs_module = {
            let spirv =
                auxil::read_spirv(Cursor::new(&include_bytes!(ps_path)[..]))
                    .unwrap();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };

        let pipeline = {
            let (vs_entry, fs_entry) = (
                EntryPoint {
                    entry: ENTRY_NAME,
                    module: &vs_module,
                    specialization: hal::spec_const_list![0.8f32],
                },
                EntryPoint {
                    entry: ENTRY_NAME,
                    module: &fs_module,
                    specialization: Specialization::default(),
                },
            );

            let subpass = Subpass {
                index: 0,
                main_pass: &*render_pass,
            };

            let vertex_buffers = vec![VertexBufferDesc {
                binding: 0,
                stride: mem::size_of::<T>() as u32,
                rate: VertexInputRate::Vertex,
            }];

            let attributes = vec![
                AttributeDesc {
                    location: 0,
                    binding: 0,
                    element: Element {
                        format: f::Format::Rg32Sfloat,
                        offset: 0,
                    },
                },
                AttributeDesc {
                    location: 1,
                    binding: 0,
                    element: Element {
                        format: f::Format::Rg32Sfloat,
                        offset: 8,
                    },
                },
            ];

            let mut pipeline_desc = GraphicsPipelineDesc::new(
                PrimitiveAssemblerDesc::Vertex {
                    buffers: &vertex_buffers,
                    attributes: &attributes,
                    input_assembler: InputAssemblerDesc {
                        primitive: Primitive::TriangleList,
                        with_adjacency: false,
                        restart_index: None,
                    },
                    vertex: vs_entry,
                    geometry: None,
                    tessellation: None,
                },
                Rasterizer::FILL,
                Some(fs_entry),
                &*pipeline_layout,
                subpass,
            );

            pipeline_desc.blender.targets.push(ColorBlendDesc {
                mask: ColorMask::ALL,
                blend: Some(BlendState::ALPHA),
            });

            unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
        };

        unsafe {
            device.destroy_shader_module(vs_module);
        }
        unsafe {
            device.destroy_shader_module(fs_module);
        }

        ManuallyDrop::new(pipeline.unwrap())
    }
}*/