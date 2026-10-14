use std::sync::Arc;
use vulkano::format::Format;
use vulkano::image::SampleCount;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageLayout, ImageUsage};

use crate::rendering::context::ChaosRenderContext;

#[derive(Debug, Clone)]
pub struct ChaosTextureDescription {
    pub name: String,
    pub dimensions: [u32; 3],
    pub mip_levels: u32,
    pub array_size: u32,
    pub format: Format,
    pub image_usage: ImageUsage,
    pub image_layout: ImageLayout,
}

#[derive(Debug, Clone)]
pub struct ChaosTexture {
    inner_image: Arc<Image>,
    inner_image_view: Arc<ImageView>,
    description: ChaosTextureDescription,
}

impl ChaosTexture {
    pub fn from_existing(name: String, image: Arc<Image>, image_layout: ImageLayout) -> Self {
        Self {
            inner_image: image.clone(),
            inner_image_view: ImageView::new_default(image.clone()).unwrap(),
            description: ChaosTextureDescription {
                name,
                dimensions: image.extent(),
                mip_levels: image.mip_levels(),
                array_size: image.array_layers(),
                format: image.format(),
                image_usage: image.usage(),
                image_layout,
            },
        }
    }

    pub fn depth_target(
        name: String,
        format: Format,
        width: u32,
        height: u32,
        image_layout: ImageLayout,
        render_context: Arc<ChaosRenderContext>,
    ) -> Self {
        Self::new(
            name,
            [width, height, 1],
            1,
            1,
            format,
            ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            image_layout,
            render_context,
        )
    }

    pub fn new_sampled(
        name: String,
        format: Format,
        width: u32,
        height: u32,
        image_layout: ImageLayout,
        render_context: Arc<ChaosRenderContext>,
    ) -> Self {
        Self::new(
            name,
            [width, height, 1],
            1,
            1,
            format,
            ImageUsage::SAMPLED,
            image_layout,
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
        image_layout: ImageLayout,
        render_context: Arc<ChaosRenderContext>,
    ) -> Self {
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
        .unwrap();

        let inner_image_view = ImageView::new_default(inner_image.clone()).unwrap();

        Self {
            inner_image,
            inner_image_view,
            description: ChaosTextureDescription {
                name,
                dimensions: dimensions,
                mip_levels: mip_levels,
                array_size: array_size,
                format: format,
                image_usage: image_usage,
                image_layout: image_layout,
            },
        }
    }

    pub fn image_view(&self) -> Arc<ImageView> {
        self.inner_image_view.clone()
    }

    pub fn image(&self) -> Arc<Image> {
        self.inner_image.clone()
    }

    pub fn get_image_layout(&self) -> ImageLayout {
        self.description.image_layout
    }

    pub fn set_image_layout(&mut self, image_layout: ImageLayout) {
        self.description.image_layout = image_layout;
    }

    pub fn description(&self) -> &ChaosTextureDescription {
        &self.description
    }
}
