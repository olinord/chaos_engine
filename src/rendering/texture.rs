use std::sync::Arc;
use vulkano::format::Format;
use vulkano::image::SampleCount;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageUsage};

use crate::rendering::context::ChaosRenderContext;
use crate::rendering::errors::{RenderError, RenderResultExt};

#[derive(Debug, Clone)]
pub struct ChaosTextureDescription {
    pub name: String,
    pub dimensions: [u32; 3],
    pub mip_levels: u32,
    pub array_size: u32,
    pub format: Format,
    pub image_usage: ImageUsage,
}

#[derive(Debug, Clone)]
pub struct ChaosTexture {
    inner_image: Arc<Image>,
    inner_image_view: Arc<ImageView>,
    description: ChaosTextureDescription,
}

impl ChaosTexture {
    pub fn from_existing(name: String, image: Arc<Image>) -> Result<Self, RenderError> {
        Ok(Self {
            inner_image: image.clone(),
            inner_image_view: ImageView::new_default(image.clone()).context("create image view")?,
            description: ChaosTextureDescription {
                name,
                dimensions: image.extent(),
                mip_levels: image.mip_levels(),
                array_size: image.array_layers(),
                format: image.format(),
                image_usage: image.usage(),
            },
        })
    }

    pub fn depth_target(
        name: String,
        format: Format,
        width: u32,
        height: u32,
        render_context: Arc<ChaosRenderContext>,
    ) -> Result<Self, RenderError> {
        Self::new(
            name,
            [width, height, 1],
            1,
            1,
            format,
            ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            render_context,
        )
    }

    pub fn new_sampled(
        name: String,
        format: Format,
        width: u32,
        height: u32,
        render_context: Arc<ChaosRenderContext>,
    ) -> Result<Self, RenderError> {
        Self::new(
            name,
            [width, height, 1],
            1,
            1,
            format,
            ImageUsage::SAMPLED,
            render_context,
        )
    }

    pub fn new(
        name: String,
        dimensions: [u32; 3],
        mip_levels: u32,
        array_size: u32,
        format: Format,
        image_usage: ImageUsage,
        render_context: Arc<ChaosRenderContext>,
    ) -> Result<Self, RenderError> {
        let create_info = ImageCreateInfo {
            format,
            extent: dimensions,
            mip_levels,
            array_layers: array_size,
            samples: SampleCount::Sample1,
            usage: image_usage,
            ..Default::default()
        };

        let inner_image = Image::new(
            render_context.memory_allocator(),
            create_info,
            Default::default(),
        )
        .map_err(|e| RenderError::allocation(name.clone(), e))?;

        let inner_image_view =
            ImageView::new_default(inner_image.clone()).context("create image view")?;

        Ok(Self {
            inner_image,
            inner_image_view,
            description: ChaosTextureDescription {
                name,
                dimensions: dimensions,
                mip_levels: mip_levels,
                array_size: array_size,
                format: format,
                image_usage: image_usage,
            },
        })
    }

    pub fn image_view(&self) -> Arc<ImageView> {
        self.inner_image_view.clone()
    }

    pub fn image(&self) -> Arc<Image> {
        self.inner_image.clone()
    }

    pub fn description(&self) -> &ChaosTextureDescription {
        &self.description
    }
}
