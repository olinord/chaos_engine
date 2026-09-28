use std::sync::Arc;
use vulkano::format::Format;
use vulkano::image::SampleCount;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageLayout, ImageUsage};

use crate::rendering::context::ChaosRenderContext;

#[derive(Debug, Clone)]
pub struct ChaosImage {
    inner_image: Arc<Image>,
    inner_image_view: Arc<ImageView>,
    pub name: String,
    pub format: Format,
    pub dimensions: [u32; 3],
    pub mip_levels: u32,
    pub array_size: u32,
    pub image_layout: ImageLayout,
}

impl ChaosImage {
    pub fn from_existing(name: String, image: Arc<Image>, image_layout: ImageLayout) -> Self {
        Self {
            inner_image: image.clone(),
            inner_image_view: ImageView::new_default(image.clone()).unwrap(),
            name,
            format: image.format(),
            dimensions: image.extent(),
            mip_levels: image.mip_levels(),
            array_size: image.array_layers(),
            image_layout,
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

    fn new(
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
            name,
            format,
            dimensions,
            mip_levels,
            array_size,
            image_layout,
        }
    }

    pub fn image_view(&self) -> Arc<ImageView> {
        self.inner_image_view.clone()
    }

    pub fn image(&self) -> Arc<Image> {
        self.inner_image.clone()
    }
}
