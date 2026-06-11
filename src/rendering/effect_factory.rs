use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{PipelineRenderingCreateInfo, PipelineSubpassType};
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::shader::spirv::bytes_to_words;
use vulkano::shader::{self, EntryPoint, ShaderModule, ShaderModuleCreateInfo};

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
    MissinglShader { error: String },
    UndefinedShaderType,
    InvalidShader { shader_path: String, error: String },
    ShaderNotFound { shader_path: String, error: String },
    MissingEntryPoint { shader_path: String },
    VulkanError { vulkan_error: String },
}

pub struct EffectUsage {
    pub path: PathBuf,
    pub viewport: vulkano::pipeline::graphics::viewport::Viewport,
    pub color_attachment_count: u32,
    pub color_attachment_format: Format,
}

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

    /// Load all shaders from the given root path.
    /// Expects files with extensions: .vert / .vs (vertex) and .frag / .ps (pixel)
    pub fn load_from_path<P: AsRef<Path>>(
        &self,
        root_path: P,
        device: &Arc<Device>,
    ) -> Result<(), String> {
        let root = root_path.as_ref();

        if !root.exists() {
            return Err(format!("Shader path does not exist: {:?}", root));
        }

        let mut shaders: HashMap<PathBuf, ShaderEntry> = HashMap::new();

        Self::walk_dir(root, &mut shaders, device)
            .map_err(|e| format!("Failed to walk shader directory: {}", e))?;

        let mut cache = self
            .shaders
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        for (_, entry) in &mut shaders.iter_mut() {
            entry.initialize(device);
        }
        cache.extend(shaders);

        Ok(())
    }

    fn walk_dir(
        current_dir: &Path,
        shaders: &mut HashMap<PathBuf, ShaderEntry>,
        device: &Arc<Device>,
    ) -> Result<(), ChaosEffectBuildError> {
        let entries = match fs::read_dir(current_dir) {
            Ok(e) => e,
            Err(e) => {
                log::info!("Failed to read shader directory: {}", e);
                return Ok(());
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
                Self::walk_dir(&path, shaders, device)?;
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

                let shader_key = path.with_file_name(shader_name);
                let entry = shaders.entry(shader_key).or_insert_with(ShaderEntry::new);

                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(ChaosEffectBuildError::ShaderNotFound {
                            shader_path: path.display().to_string(),
                            error: e.to_string(),
                        });
                    }
                };

                let words = match bytes_to_words(&bytes) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(ChaosEffectBuildError::InvalidShader {
                            shader_path: path.display().to_string(),
                            error: e.to_string(),
                        });
                    }
                };

                let vs_shader_creation = ShaderModuleCreateInfo::new(&words);
                let module = unsafe {
                    shader::ShaderModule::new(device.clone(), vs_shader_creation).map_err(|e| {
                        ChaosEffectBuildError::InvalidShader {
                            shader_path: path.display().to_string(),
                            error: e.to_string(),
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

        Ok(())
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
        device: &Arc<Device>,
    ) -> Result<Arc<GraphicsPipeline>, ChaosEffectBuildError> {
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
        if entry.is_none() {
            return Err(ChaosEffectBuildError::ShaderNotFound {
                shader_path: path.display().to_string(),
                error: "Shader not found in cache".to_string(),
            });
        }
        let entry = entry.unwrap();
        Self::create_rendering_effect::<T>(path, entry, usage, device)
    }

    fn create_rendering_effect<T: Vertex>(
        shader_path: &Path,
        shader_entry: &ShaderEntry,
        usage: &EffectUsage,
        device: &Arc<Device>,
    ) -> Result<Arc<GraphicsPipeline>, ChaosEffectBuildError> {
        let vertex_entry_point = &shader_entry
            .shaders
            .get(&ShaderType::Vertex)
            .ok_or_else(|| ChaosEffectBuildError::MissinglShader {
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
        let result = GraphicsPipeline::new(
            device.clone(),
            None,
            GraphicsPipelineCreateInfo {
                // The stages of our pipeline, we have vertex and fragment stages.
                stages: shader_entry.stages.clone().into_iter().collect(),
                // Describes the layout of the vertex input and how should it behave.
                vertex_input_state: Some(vertex_input_state),
                // Indicate the type of the primitives (the default is a list of triangles).
                input_assembly_state: Some(InputAssemblyState::default()),
                // Set the fixed viewport.
                viewport_state: Some(ViewportState {
                    viewports: [usage.viewport.clone()].into_iter().collect(),
                    ..Default::default()
                }),
                // Ignore these for now.
                rasterization_state: Some(RasterizationState::default()),
                multisample_state: Some(MultisampleState::default()),
                subpass: Some(PipelineSubpassType::BeginRendering(
                    PipelineRenderingCreateInfo {
                        color_attachment_formats: (0..usage.color_attachment_count)
                            .map(|_| Some(usage.color_attachment_format))
                            .collect(),
                        ..Default::default()
                    },
                )),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    usage.color_attachment_count,
                    ColorBlendAttachmentState::default(),
                )),
                ..GraphicsPipelineCreateInfo::layout(shader_entry.layout.clone().unwrap())
            },
        );

        match result {
            Ok(pipeline) => Ok(pipeline),
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
            ChaosEffectBuildError::MissinglShader { error } => {
                write!(f, "Missing shader: {}", error)
            }
            ChaosEffectBuildError::UndefinedShaderType => {
                write!(f, "Undefined shader type (not vertex or pixel)")
            }
            ChaosEffectBuildError::InvalidShader { shader_path, error } => {
                write!(f, "Invalid shader at path {}: {}", shader_path, error)
            }
            ChaosEffectBuildError::ShaderNotFound { shader_path, error } => {
                write!(f, "Shader not found at path {}: {}", shader_path, error)
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
