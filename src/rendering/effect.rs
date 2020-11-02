use std::mem;
use std::mem::ManuallyDrop;

use gfx_hal::Backend;
use gfx_hal::command::CommandBuffer;
use gfx_hal::device::Device;
use gfx_hal::format::Format;
use gfx_hal::pass::Subpass;
use gfx_hal::pso::{AttributeDesc, BlendState, ColorBlendDesc, ColorMask, Element, EntryPoint, GraphicsPipelineDesc, InputAssemblerDesc, Location, Primitive, PrimitiveAssemblerDesc, Rasterizer, Specialization, VertexBufferDesc, VertexInputRate, DescriptorSetLayoutBinding, ShaderStageFlags, Face};

use super::spirv_reflect::*;
use super::spirv_reflect::types::{ReflectFormat, ReflectInterfaceVariable, ReflectBlockVariable};
use std::path::PathBuf;
use std::io::Read;
use rendering::buffer::BufferData;
use std::collections::HashMap;

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
    attributes: Vec<AttributeDesc>

    // need the push constants?
}

// https://github.com/rust-tutorials/learn-gfx-hal/blob/master/examples/shaders.rs
impl<B: Backend> Effect<B>
{
    /*
    Creates a new Vertex Pixel shader effect
    */
    pub fn vertex_pixel<T: BufferData>(device: &B::Device, vs_path: String, ps_path: String, pass: &Subpass<B>) -> Result<Effect<B>, &'static str> {
        let path_to_exec = std::env::current_exe().unwrap();
        let path_to_exec_folder = path_to_exec.parent().unwrap();
        let path_to_vs: PathBuf = path_to_exec_folder.join("res/shaders").join(format!("{}.spv", vs_path));
        let path_to_ps: PathBuf = path_to_exec_folder.join("res/shaders").join(format!("{}.spv", ps_path));
        let vertex_shader = ShaderInfo::new(path_to_vs.display().to_string(), &T::layout(), pass.index as u32)?;
        let pixel_shader = ShaderInfo::new(path_to_ps.display().to_string(), &T::layout(), pass.index as u32)?;



        let bindings = Vec::<DescriptorSetLayoutBinding>::new();
        let immutable_samplers = Vec::<B::Sampler>::new();
        let descriptor_set_layouts: Vec<B::DescriptorSetLayout> = vec![unsafe {
            device
                .create_descriptor_set_layout(bindings, immutable_samplers)
                .map_err(|_| "Couldn't make a DescriptorSetLayout")?
        }];
        let push_constants = Vec::<(ShaderStageFlags, core::ops::Range<u32>)>::new();

        // push FRAGMENT 0..3
        // push VERTEX 0..4 or something

        let pipeline_layout = ManuallyDrop::new(
            unsafe {
                device
                    .create_pipeline_layout(&descriptor_set_layouts, push_constants)
            }.expect("Can't create pipeline layout")
        );


        let vertex_shader_module = unsafe {
            device.create_shader_module(vertex_shader.code.as_slice())
        }.unwrap();

        let pixel_shader_module = unsafe {
            device.create_shader_module(pixel_shader.code.as_slice())
        }.unwrap();

        let vertex_buffers = vec![VertexBufferDesc {
            binding: pass.index as u32,
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

            let mut pipeline_desc = GraphicsPipelineDesc::new(

                PrimitiveAssemblerDesc::Vertex {
                    buffers: &vertex_buffers,
                    attributes: &vertex_shader.attributes,
                    input_assembler: InputAssemblerDesc {
                        primitive: Primitive::TriangleList,
                        with_adjacency: false,
                        restart_index: None,
                    },
                    vertex: vertex_shader_entry_point,
                    geometry: None,
                    tessellation: None,
                },
                Rasterizer {
                    cull_face: Face::BACK,
                    ..Rasterizer::FILL
                },
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

            // set push constants here

            cmd_buffer.bind_graphics_descriptor_sets(&self.pipeline_layout,
                                                     0,
                                                     &[], // here should image descriptor sets
                                                     &[], );
        }
    }
}


impl ShaderInfo {
    fn new(effect_path: String, layout: &Vec<u32>, binding: u32) -> Result<ShaderInfo, &'static str> {
        println!("Reading {}", effect_path);

        let mut shader_file = std::fs::File::open(&effect_path).unwrap();
        let mut shader_data = Vec::new();

        shader_file.read_to_end(&mut shader_data).or_else(|_| Err("Failed to read"))?;

        let module = ShaderModule::load_u8_data(shader_data.as_slice()).or_else(|e| Err(format!("{}", e))).unwrap();
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
        let _constants = module.enumerate_push_constant_blocks(None).unwrap();

        let input_vars = module.enumerate_input_variables(None).unwrap();
        for var in &input_vars {
            println!(
                "   input var - name: {} location: {}",
                var.name, var.location
            );
        }
        for constant in &_constants {
            println!("    Constant - name {} offset {} size {}", constant.name, constant.offset, constant.size);
            for member in &constant.members {
                println!("    Member - name {} offset {} size {}", member.name, member.offset, member.size);
            }
        }

        return Ok(ShaderInfo {
            path: effect_path.clone(),
            entry_point_name: _entry_point_name,
            code: module.get_code(),
            attributes: ShaderInfo::get_attribute_vec(&input_vars, &layout, &effect_path, binding),
        });
    }

    fn get_attribute_vec(input_vars: &Vec<ReflectInterfaceVariable>, layout: &Vec<u32>, path: &String, binding: u32) -> Vec<AttributeDesc> {
        let mut attributes: Vec<AttributeDesc> = Vec::new();

        for attr in input_vars.as_slice() {
            attributes.push(AttributeDesc {
                location: Location::from(attr.location),
                binding: binding, // Is this for multiple buffers?
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
                            log::warn!("Unknown format for shader input {} in shader {}", attr.name, path.as_str());
                            Format::R32Uint
                        }
                    },
                    offset: layout[attr.location as usize], // have both layout and calculate, so we can assert that they are eual
                },
            });
        }
        attributes
    }

    fn get_push_constants(constants: &Vec<ReflectBlockVariable>) -> HashMap<String, u32> {
        let mut constant_map = HashMap::new();
        for constant in constants.as_slice() {
            constant_map.insert(String::from(&constant.name), constant.offset);
        }
        constant_map
    }
}