use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::input_assembly::{InputAssemblyState, PrimitiveTopology};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport::{Scissor, Viewport, ViewportState};
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{
    DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo,
};
use vulkano::shader::spirv::bytes_to_words;
use vulkano::shader::{self, EntryPoint, ShaderModule, ShaderModuleCreateInfo};

use crate::rendering::effect::ChaosEffect;
use crate::rendering::rendering_system::ChaosRenderContext;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ShaderType {
    Vertex,
    Fragment,
}

#[derive(Debug, Clone)]
struct ChaosShaderModule {
    _module: Arc<ShaderModule>,
    _shader_path: PathBuf,
    entry_point: EntryPoint,
}

#[derive(Debug, Clone)]
pub enum ChaosEffectBuildError {
    MissingDevice,
    MissingShader {
        error: String,
    },
    UndefinedShaderType,
    InvalidShader {
        shader_path: String,
        error: String,
    },
    ShaderNotFound {
        shader_path: String,
        error: String,
    },
    DirectoryNotFound {
        directory_path: String,
        error: String,
    },
    MissingEntryPoint {
        shader_path: String,
    },
    VulkanError {
        vulkan_error: String,
    },
}
#[derive(Debug, Clone)]
pub struct EffectColorAttachment {
    pub format: Format,
    pub count: u32,
}

pub struct EffectUsage {
    pub path: PathBuf,
    viewports: Vec<Viewport>,
    color_attachment: Option<EffectColorAttachment>,
    primitive_topology: PrimitiveTopology,
}

impl EffectUsage {
    pub fn new(path: PathBuf) -> Self {
        EffectUsage {
            path,
            viewports: Vec::new(),
            color_attachment: None,
            primitive_topology: PrimitiveTopology::TriangleList,
        }
    }

    pub fn with_primitive_topology(mut self, primitive_topology: PrimitiveTopology) -> Self {
        self.primitive_topology = primitive_topology;
        self
    }

    pub fn with_viewports(mut self, viewports: Vec<Viewport>) -> Self {
        self.viewports = viewports;
        self
    }

    pub fn with_color_attachment(mut self, format: Format, count: u32) -> Self {
        self.color_attachment = Some(EffectColorAttachment { format, count });
        self
    }

    pub fn viewports(&self) -> Vec<Viewport> {
        self.viewports.clone()
    }

    pub fn color_attachment(&self) -> Option<EffectColorAttachment> {
        self.color_attachment.clone()
    }
}

#[derive(Debug, Clone)]
struct ShaderEntry {
    pub shaders: HashMap<ShaderType, ChaosShaderModule>,
    pub stages: Vec<PipelineShaderStageCreateInfo>,
    pub layout: Option<Arc<PipelineLayout>>,
}

impl ShaderEntry {
    fn new() -> Self {
        ShaderEntry {
            shaders: HashMap::new(),
            stages: Vec::new(),
            layout: None,
        }
    }

    fn initialize(&mut self, device: &Arc<Device>) {
        for shader_module in self.shaders.values() {
            let entry_point = shader_module.entry_point.clone();
            self.stages
                .push(PipelineShaderStageCreateInfo::new(entry_point));
        }

        if !self.stages.is_empty() {
            self.layout = Some(
                PipelineLayout::new(
                    device.clone(),
                    PipelineDescriptorSetLayoutCreateInfo::from_stages(&self.stages)
                        .into_pipeline_layout_create_info(device.clone())
                        .unwrap(),
                )
                .unwrap(),
            );
        }
    }
}

pub struct EffectFactory {
    shaders: RwLock<HashMap<PathBuf, ShaderEntry>>,
}

static INSTANCE: OnceLock<EffectFactory> = OnceLock::new();

impl EffectFactory {
    fn new() -> Self {
        EffectFactory {
            shaders: RwLock::new(HashMap::new()),
        }
    }

    /// Get the global singleton instance
    pub fn instance() -> &'static EffectFactory {
        INSTANCE.get_or_init(EffectFactory::new)
    }

    pub fn load_from_directories(
        &self,
        directories: &HashMap<PathBuf, PathBuf>,
        render_context: &Arc<ChaosRenderContext>,
    ) {
        for (app_path, path) in directories.iter() {
            match self.load_from_path(path, render_context) {
                Ok(shaders) => {
                    for (shader_path, entry) in shaders.iter() {
                        if let Ok(relative_path) = shader_path.strip_prefix(path) {
                            // create a path that is platform independant and uses the app_path as a "drive" prefix

                            let mut path_str = format!("{}:", app_path.display());
                            relative_path
                                .components()
                                .for_each(|component| match component {
                                    std::path::Component::Normal(os_str) => {
                                        path_str +=
                                            format!("/{}", os_str.to_string_lossy()).as_str();
                                    }
                                    _ => {}
                                });

                            let modified_shader_path = PathBuf::from(path_str);

                            log::debug!("Found shader {}", modified_shader_path.display());
                            self.shaders
                                .write()
                                .unwrap()
                                .insert(modified_shader_path, entry.clone());
                        }
                    }
                    let mut shader_map = self.shaders.write().unwrap();
                    shader_map.extend(shaders);
                }
                Err(error) => {
                    log::error!(
                        "Failed to load shaders from directory {}: {}",
                        path.display(),
                        error
                    );
                }
            }
        }
    }

    /// Load all shaders from the given root path.
    /// Expects files with extensions: .vert / .vs (vertex) and .frag / .ps (pixel)
    fn load_from_path<P: AsRef<Path>>(
        &self,
        root_path: P,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<HashMap<PathBuf, ShaderEntry>, String> {
        let root = root_path.as_ref();

        if !root.exists() {
            return Err(format!("Path {} does not exist", root.display()));
        }

        let device = render_context.device();
        let mut shaders = Self::walk_dir(root, &device)
            .map_err(|e| format!("Failed to walk shader directory: {}", e))?;

        for (_, entry) in shaders.iter_mut() {
            entry.initialize(&device);
        }

        Ok(shaders)
    }

    fn walk_dir(
        current_dir: &Path,
        device: &Arc<Device>,
    ) -> Result<HashMap<PathBuf, ShaderEntry>, ChaosEffectBuildError> {
        let mut shaders: HashMap<PathBuf, ShaderEntry> = HashMap::new();
        let entries = match fs::read_dir(current_dir) {
            Ok(e) => e,
            Err(e) => {
                return Err(ChaosEffectBuildError::DirectoryNotFound {
                    directory_path: current_dir.display().to_string(),
                    error: e.to_string(),
                });
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::info!("Failed to read shader directory entry: {}", e);
                    continue;
                }
            };
            let path = entry.path();

            if path.is_dir() {
                let inner = Self::walk_dir(&path, device)?;
                shaders.extend(inner);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let shader_ext = if ext == "spv" {
                    Path::new(file_stem)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or(ext)
                } else {
                    ext
                };
                let shader_type = match shader_ext {
                    "vert" | "vs" => ShaderType::Vertex,
                    "frag" | "ps" => ShaderType::Fragment,
                    _ => continue,
                };
                let shader_name = if ext == "spv" {
                    Path::new(file_stem)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(file_stem)
                } else {
                    file_stem
                };

                if shader_name.is_empty() {
                    continue;
                }

                let shader_key = path.with_file_name(shader_name).clone();
                let entry = shaders.entry(shader_key).or_insert_with(ShaderEntry::new);

                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(ChaosEffectBuildError::ShaderNotFound {
                            shader_path: path.display().to_string(),
                            error: format!(
                                "Trying to read file {} caused error {}",
                                path.display(),
                                e
                            ),
                        });
                    }
                };

                let words = match bytes_to_words(&bytes) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(ChaosEffectBuildError::InvalidShader {
                            shader_path: path.display().to_string(),
                            error: format!(
                                "Trying to convert bytes to words for file {} caused error {}",
                                path.display(),
                                e
                            ),
                        });
                    }
                };

                let vs_shader_creation = ShaderModuleCreateInfo::new(&words);
                let module = unsafe {
                    shader::ShaderModule::new(device.clone(), vs_shader_creation).map_err(|e| {
                        ChaosEffectBuildError::InvalidShader {
                            shader_path: path.display().to_string(),
                            error: format!(
                                "Trying to create shader module for file {} caused error {}",
                                path.display(),
                                e
                            ),
                        }
                    })?
                };
                let entry_point = module.entry_point("main").ok_or_else(|| {
                    ChaosEffectBuildError::MissingEntryPoint {
                        shader_path: path.display().to_string(),
                    }
                })?;
                entry.shaders.insert(
                    shader_type,
                    ChaosShaderModule {
                        _module: module,
                        _shader_path: path.clone(),
                        entry_point,
                    },
                );
            }
        }

        Ok(shaders)
    }

    /// Returns all loaded shader names
    pub fn list(&self) -> Vec<PathBuf> {
        self.shaders
            .read()
            .map(|cache| cache.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_effect<T: Vertex>(
        &self,
        usage: &EffectUsage,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<ChaosEffect, ChaosEffectBuildError> {
        let path = &usage.path;
        let shader_map = self.shaders.read();
        if shader_map.is_err() {
            return Err(ChaosEffectBuildError::ShaderNotFound {
                shader_path: path.display().to_string(),
                error: "Failed to acquire shader cache lock".to_string(),
            });
        }

        let shader_entries = shader_map.unwrap();
        let entry = shader_entries.get(path);
        match entry {
            Some(shader_entry) => {
                Self::create_rendering_effect::<T>(path, shader_entry, usage, render_context)
            }
            None => {
                return Err(ChaosEffectBuildError::ShaderNotFound {
                    shader_path: path.display().to_string(),
                    error: format!("Shader not found in cache"),
                });
            }
        }
    }

    fn create_rendering_effect<T: Vertex>(
        shader_path: &Path,
        shader_entry: &ShaderEntry,
        usage: &EffectUsage,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<ChaosEffect, ChaosEffectBuildError> {
        let vertex_entry_point = &shader_entry
            .shaders
            .get(&ShaderType::Vertex)
            .ok_or_else(|| ChaosEffectBuildError::MissingShader {
                error: format!("Missing vertex shader for path: {}", shader_path.display()),
            })?
            .entry_point;

        let vertex_input_state = T::per_vertex().definition(vertex_entry_point);

        if let Err(e) = vertex_input_state {
            return Err(ChaosEffectBuildError::VulkanError {
                vulkan_error: format!("{:?}", e),
            });
        }

        let vertex_input_state = vertex_input_state.unwrap();
        let use_dynamic_viewport = usage.viewports.is_empty();
        // When no fixed viewports are provided, the pipeline uses dynamic
        // viewport + scissor state so it stays valid across window resizes.
        // The current viewport/scissor is written into the command buffer once
        // per frame in `ChaosRenderSystem::start_frame`.
        let viewports = if use_dynamic_viewport {
            std::iter::once(Viewport::default()).collect()
        } else {
            usage.viewports.iter().cloned().collect()
        };
        let scissors = if use_dynamic_viewport {
            std::iter::once(Scissor::default()).collect()
        } else {
            usage.viewports.iter().map(|_| Scissor::default()).collect()
        };
        let mut dynamic_state =
            GraphicsPipelineCreateInfo::layout(shader_entry.layout.clone().unwrap()).dynamic_state;
        if use_dynamic_viewport {
            dynamic_state.insert(DynamicState::Viewport);
            dynamic_state.insert(DynamicState::Scissor);
        }

        let color_attachment_format = match &usage.color_attachment {
            Some(ca) => ca.format,
            None => render_context.swapchain().image_format(),
        };

        let color_attachment_count = match &usage.color_attachment {
            Some(ca) => ca.count,
            None => 1,
        };
        let mut input_assembly_state = InputAssemblyState::default();
        input_assembly_state.topology = usage.primitive_topology;

        let result = GraphicsPipeline::new(
            render_context.device().clone(),
            None,
            GraphicsPipelineCreateInfo {
                // The stages of our pipeline, we have vertex and fragment stages.
                stages: shader_entry.stages.clone().into_iter().collect(),
                // Describes the layout of the vertex input and how should it behave.
                vertex_input_state: Some(vertex_input_state),
                // Indicate the type of the primitives (the default is a list of triangles).
                input_assembly_state: Some(input_assembly_state),
                viewport_state: Some(ViewportState {
                    viewports,
                    scissors,
                    ..Default::default()
                }),
                dynamic_state,
                // Ignore these for now.
                rasterization_state: Some(RasterizationState::default()),
                multisample_state: Some(MultisampleState::default()),
                subpass: Some(PipelineSubpassType::BeginRendering(
                    PipelineRenderingCreateInfo {
                        color_attachment_formats: (0..color_attachment_count)
                            .map(|_| Some(color_attachment_format))
                            .collect(),
                        ..Default::default()
                    },
                )),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    color_attachment_count,
                    ColorBlendAttachmentState::default(),
                )),
                ..GraphicsPipelineCreateInfo::layout(shader_entry.layout.clone().unwrap())
            },
        );

        match result {
            Ok(pipeline) => Ok(ChaosEffect::new(
                format!("{}", shader_path.display()).as_str(),
                pipeline,
                render_context.clone(),
            )),
            Err(vulkan_error) => Err(ChaosEffectBuildError::VulkanError {
                vulkan_error: format!("{:?}", vulkan_error),
            }),
        }
    }
}

impl Display for ChaosEffectBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChaosEffectBuildError::MissingDevice => write!(f, "No device provided"),
            ChaosEffectBuildError::MissingShader { error } => {
                write!(f, "Missing shader: {}", error)
            }
            ChaosEffectBuildError::UndefinedShaderType => {
                write!(f, "Undefined shader type (not vertex or pixel)")
            }
            ChaosEffectBuildError::InvalidShader { shader_path, error } => {
                write!(f, "Invalid shader at path {}: {}", shader_path, error)
            }
            ChaosEffectBuildError::ShaderNotFound { shader_path, error } => {
                write!(f, "Shader {} not found : {}", shader_path, error)
            }
            ChaosEffectBuildError::DirectoryNotFound {
                directory_path,
                error,
            } => {
                write!(
                    f,
                    "Directory not found at path {}: {}",
                    directory_path, error
                )
            }
            ChaosEffectBuildError::MissingEntryPoint { shader_path } => {
                write!(f, "Missing entry point in shader at path {}", shader_path)
            }
            ChaosEffectBuildError::VulkanError { vulkan_error } => {
                // write the vulkan error into a string and format it sensibly
                write!(f, "Vulkan error: ")?;
                vulkan_error.split('\n').for_each(|line| {
                    write!(f, "\t{}", line).unwrap();
                });
                Ok(())
            }
        }
    }
}
