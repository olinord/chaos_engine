use std::{fs, mem};
use std::any::Any;
use std::mem::ManuallyDrop;

use gfx_hal::Backend;
use gfx_hal::command::CommandBuffer;
use gfx_hal::device::Device;
use gfx_hal::format::Format;
use gfx_hal::pass::Subpass;
use gfx_hal::pso::{AttributeDesc, BlendState, ColorBlendDesc, ColorMask, Element, EntryPoint, GraphicsPipelineDesc, InputAssemblerDesc, Location, Primitive, PrimitiveAssemblerDesc, Rasterizer, Specialization, VertexBufferDesc, VertexInputRate};

use super::spirv_reflect::*;
use super::spirv_reflect::types::{ReflectFormat, ReflectInterfaceVariable};

pub struct Effect<B: Backend> {
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    vertex_shader: Option<ShaderInfo>,
    pixel_shader: Option<ShaderInfo>,
}

struct ShaderInfo {
    path: String,
    entry_point_name: String,
    code: Vec<u32>,
    attributes: Vec<ReflectInterfaceVariable>,
    // need the push constants?
}

// https://github.com/rust-tutorials/learn-gfx-hal/blob/master/examples/shaders.rs
impl<B: Backend> Effect<B>
{
    /*
    Creates a new Vertex Pixel shader effect
    */
    pub fn vertex_pixel<T: Any>(device: &B::Device, vs_path: String, ps_path: String, pass: &Subpass<B>) -> Result<Effect<B>, &'static str> {
        let pipeline_layout = ManuallyDrop::new(
            // here would the push constants be set,
            // see https://docs.rs/gfx-hal/0.6.0/gfx_hal/device/trait.Device.html#tymethod.create_pipeline_layout
            unsafe { device.create_pipeline_layout(&[], &[]) }
                .expect("Can't create pipeline layout"),
        );

        let vertex_shader = ShaderInfo::new(vs_path)?;
        let pixel_shader = ShaderInfo::new(ps_path)?;

        let vertex_shader_module = unsafe {
            device.create_shader_module(vertex_shader.code.as_slice())
        }.unwrap();

        let pixel_shader_module = unsafe {
            device.create_shader_module(pixel_shader.code.as_slice())
        }.unwrap();

        let vertex_buffers = vec![VertexBufferDesc {
            binding: 0,
            stride: mem::size_of::<T>() as u32,
            rate: VertexInputRate::Vertex, // this could also use instance, would need some different logic for that
        }];

        let graphics_pipeline = {
            let vertex_shader_entry_point = EntryPoint {
                entry: vertex_shader.entry_point_name.as_str(),
                module: &vertex_shader_module,
                specialization: Specialization::default(),
            };

            let pixel_shader_entry_point = EntryPoint {
                entry: pixel_shader.entry_point_name.as_str(),
                module: &pixel_shader_module,
                specialization: Specialization::default(),
            };

            let attributes = vertex_shader.get_attribute_vec();
            let mut pipeline_desc = GraphicsPipelineDesc::new(
                PrimitiveAssemblerDesc::Vertex {
                    buffers: &vertex_buffers,
                    attributes: &attributes,
                    input_assembler: InputAssemblerDesc {
                        primitive: Primitive::TriangleList,
                        with_adjacency: false,
                        restart_index: None,
                    },
                    vertex: vertex_shader_entry_point,
                    geometry: None,
                    tessellation: None,
                },
                Rasterizer::FILL,
                Some(pixel_shader_entry_point),
                &*pipeline_layout,
                *pass,
            );

            pipeline_desc.blender.targets.push(ColorBlendDesc {
                mask: ColorMask::ALL,
                blend: Some(BlendState::ALPHA),
            });
            unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
        };

        // kill the now obsolete things
        unsafe {
            device.destroy_shader_module(vertex_shader_module);
        }
        unsafe {
            device.destroy_shader_module(pixel_shader_module);
        }

        Ok(Effect {
            pipeline: ManuallyDrop::new(graphics_pipeline.unwrap()),
            pipeline_layout,
            vertex_shader: Some(vertex_shader),
            pixel_shader: Some(pixel_shader),
        })
    }

    pub fn bind_to_cmd_buffer(&self, cmd_buffer: &mut B::CommandBuffer) {
        unsafe {
            cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            cmd_buffer.bind_graphics_descriptor_sets(&self.pipeline_layout,
                                                     0,
                                                     &[], // here should image descriptor sets
                                                     &[], );
        }
    }
}


impl ShaderInfo {
    fn new(effect_path: String) -> Result<ShaderInfo, &'static str> {
        let shader_data = fs::read_to_string(&effect_path).or_else(|e| Err(format!("{}", e))).unwrap();

        let module = ShaderModule::load_u8_data(shader_data.as_bytes()).or_else(|e| Err(format!("{}", e))).unwrap();
        let _entry_point_name = module.get_entry_point_name();
        let _generator = module.get_generator();
        let _shader_stage = module.get_shader_stage();
        let _source_lang = module.get_source_language();
        let _source_lang_ver = module.get_source_language_version();
        let _source_file = module.get_source_file();
        let _source_text = module.get_source_text();
        let _spv_execution_model = module.get_spirv_execution_model();
        let _output_vars = module.enumerate_output_variables(None).unwrap();
        let _bindings = module.enumerate_descriptor_bindings(None).unwrap();
        let _sets = module.enumerate_descriptor_sets(None).unwrap();

        let input_vars = module.enumerate_input_variables(None).unwrap();
        for var in &input_vars {
            println!(
                "   input var - name: {} location: {}",
                var.name, var.location
            );
        }

        return Ok(ShaderInfo {
            path: effect_path,
            entry_point_name: _entry_point_name,
            code: module.get_code(),
            attributes: input_vars,
        });
    }


    fn get_attribute_vec(&self) -> Vec<AttributeDesc> {
        let mut attributes: Vec<AttributeDesc> = Vec::new();
        for attr in self.attributes.as_slice() {
            attributes.push(AttributeDesc {
                location: Location::from(attr.location),
                binding: 0, // Is this for multiple buffers?
                element: Element {
                    format: match attr.format {
                        ReflectFormat::R32_UINT => Format::R32Uint,
                        ReflectFormat::R32_SINT => Format::R32Sint,
                        ReflectFormat::R32_SFLOAT => Format::R32Sfloat,
                        ReflectFormat::R32G32_UINT => Format::Rg32Uint,
                        ReflectFormat::R32G32_SINT => Format::Rg32Sint,
                        ReflectFormat::R32G32_SFLOAT => Format::Rg32Sfloat,
                        ReflectFormat::R32G32B32_UINT => Format::Rgb32Uint,
                        ReflectFormat::R32G32B32_SINT => Format::Rgb32Sint,
                        ReflectFormat::R32G32B32_SFLOAT => Format::Rgb32Sfloat,
                        ReflectFormat::R32G32B32A32_UINT => Format::Rgba32Uint,
                        ReflectFormat::R32G32B32A32_SINT => Format::Rgba32Sint,
                        ReflectFormat::R32G32B32A32_SFLOAT => Format::Rgba32Sfloat,
                        _ => {
                            log::warn!("Unknown format for shader input {} in shader {}", attr.name, self.path.as_str());
                            Format::R32Uint
                        }
                    },
                    offset: attr.word_offset,
                },
            })
        }
        attributes
    }
}