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
    ColorAttachment,
    DepthAttachment,
    StorageWrite,
    TransferDst,
}

impl ResourceUsage {
    pub fn layout(&self) -> ImageLayout {
        match self {
            ResourceUsage::ColorAttachment => ImageLayout::ColorAttachmentOptimal,
            ResourceUsage::DepthAttachment => ImageLayout::DepthStencilAttachmentOptimal,
            ResourceUsage::StorageWrite => ImageLayout::General,
            ResourceUsage::TransferDst => ImageLayout::TransferDstOptimal,
        }
    }

    pub fn required_usage(&self) -> ImageUsage {
        match self {
            ResourceUsage::ColorAttachment => ImageUsage::COLOR_ATTACHMENT,
            ResourceUsage::DepthAttachment => ImageUsage::DEPTH_STENCIL_ATTACHMENT,
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
}
