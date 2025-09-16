use std::{fmt::Display, path::PathBuf, sync::Arc};

use vulkano::{
    device::Device,
    pipeline::{
        GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            vertex_input::{Vertex, VertexDefinition},
            viewport::{Viewport, ViewportState},
        },
        layout::PipelineDescriptorSetLayoutCreateInfo,
    },
    render_pass::{RenderPass, Subpass},
    shader,
};

#[derive(Debug, Clone)]
pub enum CEEffectType {
    Rendering,
    Compute,
    Raytracing,
}

#[derive(Debug, Clone)]
pub enum CEEffectBuildError {
    MissingDevice,
    MissingPixelShader { vertex_shader_path: String },
    MissingVertexShader { pixel_shader_path: String },
    UndefinedShaderType,
    InvalidShader { shader_path: String, error: String },
    ShaderNotFound { shader_path: String, error: String },
    MissingEntryPoint { shader_path: String },
}

#[derive(Debug, Clone)]
pub struct CEEffect {
    pub pipeline: Arc<GraphicsPipeline>,
}

#[derive(Debug, Clone)]
pub struct CEEffectBuilder {
    vertex_shader_path: Option<String>,
    vertex_entry_point: Option<String>,
    pixel_shader_path: Option<String>,
    pixel_entry_point: Option<String>,
    device: Option<Arc<Device>>,
    effect_type: CEEffectType,
    viewport: Option<Arc<Viewport>>,
    subpass: Option<Subpass>,
}

impl CEEffectBuilder {
    pub fn new(effect_type: CEEffectType) -> Self {
        CEEffectBuilder {
            vertex_shader_path: None,
            vertex_entry_point: None,
            pixel_shader_path: None,
            pixel_entry_point: None,
            device: None,
            effect_type: effect_type,
            viewport: None,
            subpass: None,
        }
    }

    fn load_shader(
        path: &String,
        entry_point: &String,
        device: &Arc<Device>,
    ) -> Result<vulkano::shader::EntryPoint, CEEffectBuildError> {
        // modify the path to be in the res/shader path
        let mut path = PathBuf::from("res/shaders").join(PathBuf::from(path));

        let path_to_exec = std::env::current_exe().unwrap();
        let path_to_exec_folder = path_to_exec.parent().unwrap();
        // and relative to the executable
        path = path_to_exec_folder.join(path);
        // and ends with spv
        path.set_extension("spv");
        println!("Loading shader from: {}", path.display());

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                return Err(CEEffectBuildError::ShaderNotFound {
                    shader_path: path.display().to_string(),
                    error: e.to_string(),
                });
            }
        };

        let words = match shader::spirv::bytes_to_words(&bytes) {
            Ok(b) => b,
            Err(e) => {
                return Err(CEEffectBuildError::InvalidShader {
                    shader_path: path.display().to_string(),
                    error: e.to_string(),
                });
            }
        };

        let vs_shader_creation = shader::ShaderModuleCreateInfo::new(&words);
        let module = unsafe {
            shader::ShaderModule::new(device.clone(), vs_shader_creation).map_err(|e| {
                CEEffectBuildError::InvalidShader {
                    shader_path: path.display().to_string(),
                    error: e.to_string(),
                }
            })?
        };
        return Ok(module.entry_point(&entry_point.as_str()).unwrap());
    }

    pub fn with_device(mut self, device: Arc<Device>) -> Self {
        self.device = Some(device);
        self
    }

    pub fn with_viewport(mut self, viewport: Arc<Viewport>) -> Self {
        self.viewport = Some(viewport);
        self
    }

    pub fn with_vertex_shader(mut self, path: String, entry_point: String) -> Self {
        self.vertex_shader_path = Some(format!("{}.spv", path));
        self.vertex_entry_point = Some(entry_point);
        self
    }

    pub fn with_pixel_shader(mut self, path: String, entry_point: String) -> Self {
        self.pixel_shader_path = Some(format!("{}.spv", path));
        self.pixel_entry_point = Some(entry_point);
        self
    }

    pub fn with_render_pass(mut self, render_pass: Arc<RenderPass>) -> Self {
        // just a dummy to make sure the render pass is there
        self.subpass = Some(Subpass::from(render_pass.clone(), 0).unwrap());
        self
    }

    fn build_rendering_effect<T: Vertex>(&self) -> Result<CEEffect, CEEffectBuildError> {
        let device = self.device.clone().unwrap().clone();

        if self.vertex_shader_path.is_none() {
            return Err(CEEffectBuildError::MissingVertexShader {
                pixel_shader_path: self.pixel_shader_path.clone().unwrap(),
            });
        }
        if self.pixel_shader_path.is_none() {
            return Err(CEEffectBuildError::MissingPixelShader {
                vertex_shader_path: self.vertex_shader_path.clone().unwrap(),
            });
        }
        if self.vertex_entry_point.is_none() {
            return Err(CEEffectBuildError::MissingEntryPoint {
                shader_path: self.vertex_shader_path.clone().unwrap(),
            });
        }
        if self.pixel_entry_point.is_none() {
            return Err(CEEffectBuildError::MissingEntryPoint {
                shader_path: self.pixel_shader_path.clone().unwrap(),
            });
        }

        let viewport: Viewport = self.viewport.clone().unwrap().as_ref().clone();

        let vs_entry = Self::load_shader(
            &self.vertex_shader_path.clone().unwrap(),
            &self.vertex_entry_point.clone().unwrap(),
            &device,
        )?;

        let ps_entry = Self::load_shader(
            &self.pixel_shader_path.clone().unwrap(),
            &self.pixel_entry_point.clone().unwrap(),
            &device,
        )?;

        // pull out the vertex definition
        let vertex_input_state = T::per_vertex().definition(&vs_entry).unwrap();

        let stages = [
            PipelineShaderStageCreateInfo::new(vs_entry),
            PipelineShaderStageCreateInfo::new(ps_entry),
        ];

        // create the pipeline layout
        let layout = PipelineLayout::new(
            device.clone(),
            PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                .into_pipeline_layout_create_info(device.clone())
                .unwrap(),
        )
        .unwrap();

        let sub_pass = self.subpass.clone().unwrap();
        let gp = GraphicsPipeline::new(
            device.clone(),
            None,
            GraphicsPipelineCreateInfo {
                // The stages of our pipeline, we have vertex and fragment stages.
                stages: stages.into_iter().collect(),
                // Describes the layout of the vertex input and how should it behave.
                vertex_input_state: Some(vertex_input_state),
                // Indicate the type of the primitives (the default is a list of triangles).
                input_assembly_state: Some(InputAssemblyState::default()),
                // Set the fixed viewport.
                viewport_state: Some(ViewportState {
                    viewports: [viewport].into_iter().collect(),
                    ..Default::default()
                }),
                // Ignore these for now.
                rasterization_state: Some(RasterizationState::default()),
                multisample_state: Some(MultisampleState::default()),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    (&sub_pass).num_color_attachments(),
                    ColorBlendAttachmentState::default(),
                )),
                // This graphics pipeline object concerns the first pass of the render pass.
                subpass: Some(sub_pass.into()),
                ..GraphicsPipelineCreateInfo::layout(layout)
            },
        )
        .unwrap();
        Ok(CEEffect { pipeline: gp })
    }

    pub fn build<T: Vertex>(self) -> Result<CEEffect, CEEffectBuildError> {
        if self.device.is_none() {
            return Err(CEEffectBuildError::MissingDevice);
        }

        match self.effect_type {
            CEEffectType::Rendering => self.build_rendering_effect::<T>(),
            CEEffectType::Compute => {
                unimplemented!("Compute effects")
            }
            CEEffectType::Raytracing => {
                unimplemented!("Raytracing effects")
            }
        }
    }
}

impl Display for CEEffectBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CEEffectBuildError::MissingDevice => write!(f, "No device specified for effect"),
            CEEffectBuildError::MissingPixelShader { vertex_shader_path } => {
                write!(
                    f,
                    "No pixel shader specified for effect with vertex shader '{}'",
                    vertex_shader_path
                )
            }
            CEEffectBuildError::MissingVertexShader { pixel_shader_path } => {
                write!(
                    f,
                    "No vertex shader specified for effect with pixel shader '{}'",
                    pixel_shader_path
                )
            }
            CEEffectBuildError::UndefinedShaderType => {
                write!(
                    f,
                    "No effect type specified (rendering, compute, raytracing)"
                )
            }
            CEEffectBuildError::InvalidShader { shader_path, error } => {
                write!(f, "Invalid shader '{}': {}", shader_path, error)
            }
            CEEffectBuildError::ShaderNotFound { shader_path, error } => {
                write!(f, "Shader not found '{}': {}", shader_path, error)
            }
            CEEffectBuildError::MissingEntryPoint { shader_path } => {
                write!(f, "No entry point specified for shader '{}'", shader_path)
            }
        }
    }
}

/*use std::any::Any;
use std::mem::ManuallyDrop;

use gfx_hal::Backend;
use gfx_hal::command::CommandBuffer;
use gfx_hal::device::Device;
use gfx_hal::format::Format;
use gfx_hal::pass::Subpass;
use gfx_hal::pso::{AttributeDesc, BlendState, ColorBlendDesc, ColorMask, Element, EntryPoint, GraphicsPipelineDesc, InputAssemblerDesc, Location, Primitive, PrimitiveAssemblerDesc, Rasterizer, Specialization, VertexBufferDesc, VertexInputRate, DescriptorSetLayoutBinding, ShaderStageFlags, Face};

use super::spirv_reflect::*;
use super::spirv_reflect::types::{ReflectFormat, ReflectInterfaceVariable};
use std::path::PathBuf;
use std::io::Read;
use std::ops::Range;

pub struct Effect<B: Backend, T: Any> {
    vertex_shader_path: Option<String>,
    pixel_shader_path: Option<String>,
    pipeline: Option<ManuallyDrop<B::GraphicsPipeline>>,
    pipeline_layout: Option<ManuallyDrop<B::PipelineLayout>>,
    vertex_shader: Option<ShaderInfo>,
    pixel_shader: Option<ShaderInfo>,
    const_parameter: Option<T>,
    stride: usize,
    shader_layout: Vec<u32>,
    initialized: bool
}

struct ShaderInfo {
    #[allow(unused)]
    path: String,
    entry_point_name: String,
    code: Vec<u32>,
    attributes: Vec<AttributeDesc>
}

// https://www.falseidolfactory.com/2020/04/01/intro-to-gfx-hal-part-2-push-constants.html
impl<B: Backend, T: Any> Effect<B, T>
{
    pub fn new_vs_ps(vs_path: String, ps_path: String, stride: usize, shader_layout: Vec<u32>) -> Effect<B, T> {
        return Effect {
            vertex_shader_path: Some(vs_path),
            pixel_shader_path: Some(ps_path),
            pipeline: None,
            pipeline_layout: None,
            vertex_shader: None,
            pixel_shader: None,
            const_parameter: None,
            stride,
            shader_layout,
            initialized: false
        }
    }

    pub fn initialize(&mut self, device: &B::Device, pass: &Subpass<B>) -> Result<(), &'static str> {
        if self.initialized {
            return Ok(())
        }
        let path_to_exec = std::env::current_exe().unwrap();
        let path_to_exec_folder = path_to_exec.parent().unwrap();
        let path_to_vs: PathBuf = path_to_exec_folder.join("res/shaders").join(format!("{}.spv", self.vertex_shader_path.as_ref().unwrap()));
        let path_to_ps: PathBuf = path_to_exec_folder.join("res/shaders").join(format!("{}.spv", self.pixel_shader_path.as_ref().unwrap()));
        let vertex_shader = ShaderInfo::new(path_to_vs.display().to_string(), &self.shader_layout, pass.index as u32)?;
        let pixel_shader = ShaderInfo::new(path_to_ps.display().to_string(), &self.shader_layout, pass.index as u32)?;

        let bindings = Vec::<DescriptorSetLayoutBinding>::new();
        let immutable_samplers = Vec::<B::Sampler>::new();
        let descriptor_set_layouts: Vec<B::DescriptorSetLayout> = vec![unsafe {
            device
                .create_descriptor_set_layout(bindings, immutable_samplers)
                .map_err(|_| "Couldn't make a DescriptorSetLayout")?
        }];
        let mut push_constants = Vec::<(ShaderStageFlags, Range<u32>)>::new();
        let push_constant_bytes = std::mem::size_of::<T>() as u32;

        push_constants.push((ShaderStageFlags::VERTEX, 0..push_constant_bytes));
        push_constants.push((ShaderStageFlags::FRAGMENT, 0..push_constant_bytes));

        let pipeline_layout = ManuallyDrop::new(
            unsafe {
                device
                    .create_pipeline_layout(&descriptor_set_layouts, push_constants.to_vec())
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
            stride: self.stride as u32,
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

        self.pipeline = Some(ManuallyDrop::new(graphics_pipeline.unwrap()));
        self.pipeline_layout = Some(pipeline_layout);
        self.vertex_shader = Some(vertex_shader);
        self.pixel_shader = Some(pixel_shader);
        self.initialized = true;
        return Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        return self.initialized;
    }

    pub fn set_push_constant(&mut self, constant: T) {
        self.const_parameter = Some(constant);
    }

    unsafe fn convert_const_parameter(&self) -> &[u32]
    {
        if self.const_parameter.is_none(){
            return &[0];
        }
        let size_in_bytes = std::mem::size_of::<T>();
        let size_in_u32s = size_in_bytes / std::mem::size_of::<u32>();
        let start_ptr = self.const_parameter.as_ref().unwrap() as *const T as *const u32;
        std::slice::from_raw_parts(start_ptr, size_in_u32s)
    }


    pub fn bind_to_cmd_buffer(&self, cmd_buffer: &mut B::CommandBuffer) {
        if !self.initialized {
            println!("Trying to bind un-initialized effect to command buffer. Doing nothing instead");
            return;
        }
        unsafe {
            cmd_buffer.bind_graphics_pipeline(self.pipeline.as_ref().unwrap());

            // set push constants here
            cmd_buffer.push_graphics_constants(
                self.pipeline_layout.as_ref().unwrap(),
                ShaderStageFlags::VERTEX,
                0,
                self.convert_const_parameter()
            );

            cmd_buffer.push_graphics_constants(
                self.pipeline_layout.as_ref().unwrap(),
                ShaderStageFlags::FRAGMENT,
                0,
                self.convert_const_parameter()
            );

            cmd_buffer.bind_graphics_descriptor_sets(self.pipeline_layout.as_ref().unwrap(),
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
        let constants = module.enumerate_push_constant_blocks(None).unwrap();

        let input_vars = module.enumerate_input_variables(None).unwrap();
        for var in &input_vars {
            println!(
                "\tinput var - name: {} location: {}",
                var.name, var.location
            );
        }
        for constant in &constants {
            println!("\tConstant - name {} offset {} size {}", constant.name, constant.offset, constant.size);
            for member in &constant.members {
                println!("\t\tMember - name {} offset {} size {}", member.name, member.offset, member.size);
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
                binding, // Is this for multiple buffers?
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
}*/
