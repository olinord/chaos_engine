use std::sync::Arc;

use vulkano::{
    format::Format,
    image::{ImageLayout, ImageUsage},
    sync::{AccessFlags, PipelineStages},
};

use crate::rendering::{
    buffer::ChaosBuffer,
    command_buffer::{ChaosRenderAttachment, ChaosRenderAttachmentLoadOp},
    context::ChaosRenderContext,
    errors::RenderError,
    texture::ChaosTexture,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureSizeType {
    Fixed,
    PercentageOfRenderWidth,
    PercentageOfRenderHeight,
    PercentageOfDisplayWidth,
    PercentageOfDisplayHeight,
}

#[derive(Debug, Clone)]
pub struct TextureDescription {
    pub format: Format,
    pub dimensions: [u32; 3],
    pub mip_levels: u32,
    pub array_size: u32,
    pub dimension_calculations: [TextureSizeType; 3],
    pub usage: ImageUsage,
}

#[derive(Debug, Clone)]
pub enum ResourceUsage {
    ColorAttachment(ChaosRenderAttachmentLoadOp),
    DepthAttachment(ChaosRenderAttachmentLoadOp),
    StorageWrite,
    TransferDst,
}

impl ResourceUsage {
    pub fn layout(&self) -> ImageLayout {
        match self {
            ResourceUsage::ColorAttachment(_) => ImageLayout::ColorAttachmentOptimal,
            ResourceUsage::DepthAttachment(_) => ImageLayout::DepthStencilAttachmentOptimal,
            ResourceUsage::StorageWrite => ImageLayout::General,
            ResourceUsage::TransferDst => ImageLayout::TransferDstOptimal,
        }
    }

    pub fn required_usage(&self) -> ImageUsage {
        match self {
            ResourceUsage::ColorAttachment(_) => ImageUsage::COLOR_ATTACHMENT,
            ResourceUsage::DepthAttachment(_) => ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            ResourceUsage::StorageWrite => ImageUsage::STORAGE,
            ResourceUsage::TransferDst => ImageUsage::TRANSFER_DST,
        }
    }
}

/// The pipeline stages, read accesses and write accesses an image can be involved in while it
/// is in `layout`. The graph doesn't know which shader stage a node uses, so shader layouts
/// cover every stage that can touch them.
/// The previous layout only contributes its writes to a barrier (reads need no availability
/// operation), the new layout contributes everything it needs to see.
pub fn layout_scope(layout: ImageLayout) -> Option<(PipelineStages, AccessFlags, AccessFlags)> {
    let none = AccessFlags::empty();
    Some(match layout {
        // nothing to make available, but wait for earlier users of the memory
        ImageLayout::Undefined => (PipelineStages::ALL_COMMANDS, none, none),
        ImageLayout::ColorAttachmentOptimal => (
            PipelineStages::COLOR_ATTACHMENT_OUTPUT,
            AccessFlags::COLOR_ATTACHMENT_READ,
            AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        ImageLayout::DepthStencilAttachmentOptimal => (
            PipelineStages::EARLY_FRAGMENT_TESTS | PipelineStages::LATE_FRAGMENT_TESTS,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ),
        ImageLayout::DepthStencilReadOnlyOptimal => (
            PipelineStages::EARLY_FRAGMENT_TESTS
                | PipelineStages::LATE_FRAGMENT_TESTS
                | PipelineStages::FRAGMENT_SHADER,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ | AccessFlags::SHADER_SAMPLED_READ,
            none,
        ),
        ImageLayout::ShaderReadOnlyOptimal => (
            PipelineStages::FRAGMENT_SHADER | PipelineStages::COMPUTE_SHADER,
            AccessFlags::SHADER_SAMPLED_READ,
            none,
        ),
        ImageLayout::TransferSrcOptimal => (
            PipelineStages::ALL_TRANSFER,
            AccessFlags::TRANSFER_READ,
            none,
        ),
        ImageLayout::TransferDstOptimal => (
            PipelineStages::ALL_TRANSFER,
            none,
            AccessFlags::TRANSFER_WRITE,
        ),
        ImageLayout::General => (
            PipelineStages::ALL_COMMANDS,
            AccessFlags::MEMORY_READ,
            AccessFlags::MEMORY_WRITE,
        ),
        ImageLayout::PresentSrc => (PipelineStages::BOTTOM_OF_PIPE, none, none),
        _ => {
            return None;
        }
    })
}

/// The `ImageUsage` an image must have been created with to be used in `layout`. Empty for
/// layouts that need no usage flag (`Undefined`, `PresentSrc`, ...).
///
/// `DepthStencilReadOnlyOptimal` maps to `SAMPLED`, because the graph only uses it for node
/// inputs, which are shader reads. A read-only depth *attachment* would need
/// `DEPTH_STENCIL_ATTACHMENT` instead.
///
/// For every `ResourceUsage`, `layout_usage(usage.layout())` equals `usage.required_usage()`.
pub fn layout_usage(layout: ImageLayout) -> ImageUsage {
    match layout {
        ImageLayout::ShaderReadOnlyOptimal => ImageUsage::SAMPLED,
        ImageLayout::DepthStencilReadOnlyOptimal => ImageUsage::SAMPLED,
        ImageLayout::General => ImageUsage::STORAGE,
        ImageLayout::TransferDstOptimal => ImageUsage::TRANSFER_DST,
        ImageLayout::TransferSrcOptimal => ImageUsage::TRANSFER_SRC,
        ImageLayout::ColorAttachmentOptimal => ImageUsage::COLOR_ATTACHMENT,
        ImageLayout::DepthStencilAttachmentOptimal => ImageUsage::DEPTH_STENCIL_ATTACHMENT,
        _ => ImageUsage::empty(),
    }
}

/// Whether moving an image from `old` to `new` needs a pipeline barrier.
///
/// A layout change always does. Staying in the same layout needs one too when that layout
/// allows writes (per `layout_scope`): two color attachment writes, or a `General` storage
/// write followed by a read, are hazards even though nothing transitions. Only read after read
/// in a read-only layout, like two `ShaderReadOnlyOptimal` samplers, can skip the barrier.
///
/// Nodes don't declare their real access yet, so a writable layout is treated as "the last use
/// may have written". That's conservative: two readers in `General` still get a barrier.
pub fn needs_barrier(old: ImageLayout, new: ImageLayout) -> bool {
    if old != new {
        return true;
    }
    match layout_scope(old) {
        Some((_stage, _read_access, write_access)) => !write_access.is_empty(),
        None => {
            // this is an invalid state that makes create_barrier panic with a message naming the layout
            true
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportedResourceSetting {
    pub initial_layout: ImageLayout,
    pub end_layout: ImageLayout,
}

#[derive(Debug, Clone)]
pub enum ResourceDescription {
    Texture(TextureDescription),
    Imported {
        resource_name: String,
        setting: ImportedResourceSetting,
    },
    #[cfg(test)]
    TestTextureResource,
    #[cfg(test)]
    TestBufferResource,
}

#[derive(Debug, Clone)]
pub struct ResourceHandle {
    pub name: String,
    pub description: ResourceDescription,
    pub resource: Option<Arc<Resource>>,
}

#[derive(Debug)]
pub enum Resource {
    Texture(Arc<ChaosTexture>),
    Buffer(Arc<ChaosBuffer>),
    #[cfg(test)]
    TestTextureResource,
    #[cfg(test)]
    TestBufferResource,
}

impl ResourceHandle {
    pub fn new(desc: &ResourceDescription, name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: desc.clone(),
            resource: None,
        }
    }
    pub fn new_with_texture(
        desc: &ResourceDescription,
        name: &str,
        texture: &Arc<ChaosTexture>,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: desc.clone(),
            resource: Some(Arc::new(Resource::Texture(texture.clone()))),
        }
    }

    pub fn as_attachment(
        &self,
        load_op: ChaosRenderAttachmentLoadOp,
        layout: ImageLayout,
        store: bool,
    ) -> Arc<ChaosRenderAttachment> {
        let texture = match &self.resource {
            Some(resource) => resource,
            None => panic!("trying to use a None resource as an attachment"),
        };

        match texture.as_ref() {
            Resource::Texture(chaos_texture) => Arc::new(ChaosRenderAttachment::new(
                chaos_texture.clone(),
                load_op,
                layout,
                store,
            )),
            _ => panic!("trying to use a non-texture resource as an texture attachment"),
        }
    }

    pub fn recreate_resource(
        &mut self,
        display_size: [u32; 2],
        render_size: [u32; 2],
        render_context: Arc<ChaosRenderContext>,
    ) -> Result<(), RenderError> {
        match &self.description {
            ResourceDescription::Texture(desc) => {
                let mut dimensions = [0u32; 3];
                for i in 0..3 {
                    dimensions[i] = match desc.dimension_calculations[i] {
                        TextureSizeType::Fixed => desc.dimensions[i],
                        TextureSizeType::PercentageOfDisplayHeight => {
                            // Replace with actual display height
                            (display_size[1] * desc.dimensions[i]) / 100
                        }
                        TextureSizeType::PercentageOfDisplayWidth => {
                            // Replace with actual display width
                            (display_size[0] * desc.dimensions[i]) / 100
                        }
                        TextureSizeType::PercentageOfRenderHeight => {
                            // Replace with actual render height
                            (render_size[1] * desc.dimensions[i]) / 100
                        }
                        TextureSizeType::PercentageOfRenderWidth => {
                            // Replace with actual render width
                            (render_size[0] * desc.dimensions[i]) / 100
                        }
                    };
                }

                let image = ChaosTexture::new(
                    self.name.clone(),
                    dimensions,
                    desc.mip_levels,
                    desc.array_size,
                    desc.format,
                    desc.usage,
                    render_context,
                )?;
                self.resource = Some(Arc::new(Resource::Texture(Arc::new(image))));
            }
            ResourceDescription::Imported { .. } => {}
            #[cfg(test)]
            ResourceDescription::TestTextureResource => {
                self.resource = Some(Arc::new(Resource::TestTextureResource));
            }
            #[cfg(test)]
            ResourceDescription::TestBufferResource => {
                self.resource = Some(Arc::new(Resource::TestBufferResource));
            }
        }
        Ok(())
    }

    pub fn get_or_create_resource(
        &mut self,
        display_size: [u32; 2],
        render_size: [u32; 2],
        render_context: Arc<ChaosRenderContext>,
    ) -> Result<Arc<Resource>, RenderError> {
        if let Some(resource) = &self.resource {
            return Ok(resource.clone());
        }

        self.recreate_resource(display_size, render_size, render_context)?;

        Ok(self
            .resource
            .clone()
            .expect("recreate_resource sets the resource on success"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_scope_writes_only_for_writable_layouts() {
        let (_, _, shader_writes) = layout_scope(ImageLayout::ShaderReadOnlyOptimal).unwrap();
        let (_, _, color_writes) = layout_scope(ImageLayout::ColorAttachmentOptimal).unwrap();

        assert!(shader_writes.is_empty());
        assert_eq!(color_writes, AccessFlags::COLOR_ATTACHMENT_WRITE);
        assert!(layout_scope(ImageLayout::PresentSrc).is_some());
        assert!(layout_scope(ImageLayout::Preinitialized).is_none());
    }

    /// Layouts a use may write in, so a later use in the same layout has a hazard.
    const WRITABLE_LAYOUTS: [ImageLayout; 4] = [
        ImageLayout::ColorAttachmentOptimal,
        ImageLayout::DepthStencilAttachmentOptimal,
        ImageLayout::General,
        ImageLayout::TransferDstOptimal,
    ];

    /// Layouts that can only be read in, so staying in them is read after read.
    const READ_ONLY_LAYOUTS: [ImageLayout; 4] = [
        ImageLayout::ShaderReadOnlyOptimal,
        ImageLayout::DepthStencilReadOnlyOptimal,
        ImageLayout::TransferSrcOptimal,
        ImageLayout::PresentSrc,
    ];

    #[test]
    fn test_same_writable_layout_needs_barrier() {
        // e.g. one node clears the backbuffer and the next one loads it
        for layout in WRITABLE_LAYOUTS {
            assert!(needs_barrier(layout, layout), "{layout:?} -> {layout:?}");
        }
    }

    #[test]
    fn test_same_read_only_layout_needs_no_barrier() {
        for layout in READ_ONLY_LAYOUTS {
            assert!(!needs_barrier(layout, layout), "{layout:?} -> {layout:?}");
        }
    }

    #[test]
    fn test_layout_change_always_needs_barrier() {
        let layouts: Vec<ImageLayout> = WRITABLE_LAYOUTS
            .into_iter()
            .chain(READ_ONLY_LAYOUTS)
            .collect();
        for &old in &layouts {
            for &new in &layouts {
                if old != new {
                    assert!(needs_barrier(old, new), "{old:?} -> {new:?}");
                }
            }
        }
    }

    #[test]
    fn test_first_use_after_undefined_needs_barrier() {
        // imports with an Undefined initial layout and freshly created textures start here
        for layout in WRITABLE_LAYOUTS.into_iter().chain(READ_ONLY_LAYOUTS) {
            assert!(
                needs_barrier(ImageLayout::Undefined, layout),
                "Undefined -> {layout:?}"
            );
        }
    }

    #[test]
    fn test_layout_without_scope_needs_barrier() {
        // sort rejects these, so needs_barrier says yes and create_barrier panics on them
        assert!(needs_barrier(
            ImageLayout::Preinitialized,
            ImageLayout::Preinitialized
        ));
    }

    #[test]
    fn test_writable_layouts_match_layout_scope() {
        // keeps the lists above honest if layout_scope changes
        for layout in WRITABLE_LAYOUTS {
            let (_, _, writes) = layout_scope(layout).unwrap();
            assert!(!writes.is_empty(), "{layout:?} should allow writes");
        }
        for layout in READ_ONLY_LAYOUTS {
            let (_, _, writes) = layout_scope(layout).unwrap();
            assert!(writes.is_empty(), "{layout:?} should be read-only");
        }
    }

    #[test]
    fn test_layout_usage_table() {
        let expected = [
            (ImageLayout::ShaderReadOnlyOptimal, ImageUsage::SAMPLED),
            (ImageLayout::DepthStencilReadOnlyOptimal, ImageUsage::SAMPLED),
            (ImageLayout::General, ImageUsage::STORAGE),
            (ImageLayout::TransferSrcOptimal, ImageUsage::TRANSFER_SRC),
            (ImageLayout::TransferDstOptimal, ImageUsage::TRANSFER_DST),
            (
                ImageLayout::ColorAttachmentOptimal,
                ImageUsage::COLOR_ATTACHMENT,
            ),
            (
                ImageLayout::DepthStencilAttachmentOptimal,
                ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            ),
            (ImageLayout::Undefined, ImageUsage::empty()),
            (ImageLayout::PresentSrc, ImageUsage::empty()),
        ];
        for (layout, usage) in expected {
            assert_eq!(layout_usage(layout), usage, "{layout:?}");
        }
    }

    #[test]
    fn test_layout_usage_matches_required_usage() {
        // two tables describe the same thing; this keeps them from drifting apart
        let usages = [
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            ResourceUsage::DepthAttachment(ChaosRenderAttachmentLoadOp::DontCare),
            ResourceUsage::StorageWrite,
            ResourceUsage::TransferDst,
        ];
        for usage in usages {
            assert_eq!(
                layout_usage(usage.layout()),
                usage.required_usage(),
                "{usage:?}"
            );
        }
    }
}
