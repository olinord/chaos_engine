use std::sync::Arc;

use vulkano::{
    format::Format,
    image::{ImageLayout, ImageUsage},
};

use crate::rendering::{buffer::ChaosBuffer, context::ChaosRenderContext, texture::ChaosTexture};

#[derive(Debug, Clone)]
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
    pub image_layout: ImageLayout,
    pub dimension_calculations: [TextureSizeType; 3],
    pub usage: ImageUsage,
}

#[derive(Debug, Clone)]
pub enum ResourceDescription {
    Texture(TextureDescription),
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

    pub fn recreate_resource(
        &mut self,
        display_size: [u32; 2],
        render_size: [u32; 2],
        render_context: Arc<ChaosRenderContext>,
    ) {
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
                    desc.image_layout,
                    render_context,
                );
                self.resource = Some(Arc::new(Resource::Texture(Arc::new(image))));
            }
            #[cfg(test)]
            ResourceDescription::TestTextureResource => {
                self.resource = Some(Arc::new(Resource::TestTextureResource));
            }
            #[cfg(test)]
            ResourceDescription::TestBufferResource => {
                self.resource = Some(Arc::new(Resource::TestBufferResource));
            }
        }
    }

    pub fn get_or_create_resource(
        &mut self,
        display_size: [u32; 2],
        render_size: [u32; 2],
        render_context: Arc<ChaosRenderContext>,
    ) -> Arc<Resource> {
        if let Some(resource) = &self.resource {
            return resource.clone();
        }

        self.recreate_resource(display_size, render_size, render_context);

        self.resource.as_ref().unwrap().clone()
    }
}
