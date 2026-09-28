use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use vulkano::image::sampler::{
    BorderColor as VkBorderColor, Filter as VkFilter, Sampler, SamplerAddressMode,
    SamplerCreateInfo, SamplerMipmapMode,
};
use vulkano::pipeline::graphics::depth_stencil::CompareOp as VkCompareOp;

use crate::rendering::context::ChaosRenderContext;

#[derive(Debug)]
pub struct ChaosSampler {
    pub vk_sampler: Arc<Sampler>,
    pub vk_sampler_create_info: SamplerCreateInfo,
}

/// Special LOD value meaning "no upper clamp" — mirrors Vulkan's `LOD_CLAMP_NONE` (1000.0),
/// redeclared here so callers don't need a vulkano import for it.
pub const LOD_CLAMP_NONE: f32 = 1000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MipmapMode {
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirroredRepeat,
    ClampToBorder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompareOp {
    Never,
    Less,
    Equal,
    LessOrEqual,
    Greater,
    NotEqual,
    GreaterOrEqual,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorderColor {
    FloatTransparentBlack,
    FloatOpaqueBlack,
    FloatOpaqueWhite,
    IntTransparentBlack,
    IntOpaqueBlack,
    IntOpaqueWhite,
}

/// Backend-agnostic, hashable description of a sampler's configuration. This is both
/// the shape handed to the Vulkan backend and the cache key below, so any combination
/// of settings gets deduplicated to a single `VkSampler` — not just the three named
/// presets. Build one directly (with `..SamplerDescriptor::LINEAR_CLAMP` etc. as a
/// starting point) for anisotropic, comparison (shadow), or border-color samplers.
#[derive(Debug, Clone, Copy)]
pub struct SamplerDescriptor {
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_mode: MipmapMode,
    pub address_mode: [AddressMode; 3],
    pub max_anisotropy: Option<f32>,
    pub min_lod: f32,
    pub max_lod: f32,
    pub compare: Option<CompareOp>,
    pub border_color: BorderColor,
}

impl SamplerDescriptor {
    pub const LINEAR_CLAMP: Self = Self {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_mode: MipmapMode::Linear,
        address_mode: [AddressMode::ClampToEdge; 3],
        max_anisotropy: None,
        min_lod: 0.0,
        max_lod: 0.0,
        compare: None,
        border_color: BorderColor::FloatTransparentBlack,
    };

    pub const NEAREST_CLAMP: Self = Self {
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_mode: MipmapMode::Nearest,
        address_mode: [AddressMode::ClampToEdge; 3],
        max_anisotropy: None,
        min_lod: 0.0,
        max_lod: 0.0,
        compare: None,
        border_color: BorderColor::FloatTransparentBlack,
    };

    pub const LINEAR_REPEAT: Self = Self {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_mode: MipmapMode::Linear,
        address_mode: [AddressMode::Repeat; 3],
        max_anisotropy: None,
        min_lod: 0.0,
        max_lod: 0.0,
        compare: None,
        border_color: BorderColor::FloatTransparentBlack,
    };

    // `mip_lod_bias` is engine-wide (from `ChaosRenderContext`), not part of the descriptor.
    fn to_vulkano(self, mip_lod_bias: f32) -> SamplerCreateInfo {
        SamplerCreateInfo {
            mag_filter: self.mag_filter.into(),
            min_filter: self.min_filter.into(),
            mipmap_mode: self.mipmap_mode.into(),
            address_mode: self.address_mode.map(Into::into),
            mip_lod_bias,
            anisotropy: self.max_anisotropy,
            compare: self.compare.map(Into::into),
            lod: self.min_lod..=self.max_lod,
            border_color: self.border_color.into(),
            ..Default::default()
        }
    }
}

// f32 fields can't derive Eq/Hash, so compare/hash by bit pattern instead — sampler
// descriptors are static, engine-authored data and are never NaN, so this is safe.
impl PartialEq for SamplerDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.mag_filter == other.mag_filter
            && self.min_filter == other.min_filter
            && self.mipmap_mode == other.mipmap_mode
            && self.address_mode == other.address_mode
            && self.max_anisotropy.map(f32::to_bits) == other.max_anisotropy.map(f32::to_bits)
            && self.min_lod.to_bits() == other.min_lod.to_bits()
            && self.max_lod.to_bits() == other.max_lod.to_bits()
            && self.compare == other.compare
            && self.border_color == other.border_color
    }
}

impl Eq for SamplerDescriptor {}

impl Hash for SamplerDescriptor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.mag_filter.hash(state);
        self.min_filter.hash(state);
        self.mipmap_mode.hash(state);
        self.address_mode.hash(state);
        self.max_anisotropy.map(f32::to_bits).hash(state);
        self.min_lod.to_bits().hash(state);
        self.max_lod.to_bits().hash(state);
        self.compare.hash(state);
        self.border_color.hash(state);
    }
}

impl From<FilterMode> for VkFilter {
    fn from(value: FilterMode) -> Self {
        match value {
            FilterMode::Nearest => VkFilter::Nearest,
            FilterMode::Linear => VkFilter::Linear,
        }
    }
}

impl From<MipmapMode> for SamplerMipmapMode {
    fn from(value: MipmapMode) -> Self {
        match value {
            MipmapMode::Nearest => SamplerMipmapMode::Nearest,
            MipmapMode::Linear => SamplerMipmapMode::Linear,
        }
    }
}

impl From<AddressMode> for SamplerAddressMode {
    fn from(value: AddressMode) -> Self {
        match value {
            AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
            AddressMode::Repeat => SamplerAddressMode::Repeat,
            AddressMode::MirroredRepeat => SamplerAddressMode::MirroredRepeat,
            AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
        }
    }
}

impl From<CompareOp> for VkCompareOp {
    fn from(value: CompareOp) -> Self {
        match value {
            CompareOp::Never => VkCompareOp::Never,
            CompareOp::Less => VkCompareOp::Less,
            CompareOp::Equal => VkCompareOp::Equal,
            CompareOp::LessOrEqual => VkCompareOp::LessOrEqual,
            CompareOp::Greater => VkCompareOp::Greater,
            CompareOp::NotEqual => VkCompareOp::NotEqual,
            CompareOp::GreaterOrEqual => VkCompareOp::GreaterOrEqual,
            CompareOp::Always => VkCompareOp::Always,
        }
    }
}

impl From<BorderColor> for VkBorderColor {
    fn from(value: BorderColor) -> Self {
        match value {
            BorderColor::FloatTransparentBlack => VkBorderColor::FloatTransparentBlack,
            BorderColor::FloatOpaqueBlack => VkBorderColor::FloatOpaqueBlack,
            BorderColor::FloatOpaqueWhite => VkBorderColor::FloatOpaqueWhite,
            BorderColor::IntTransparentBlack => VkBorderColor::IntTransparentBlack,
            BorderColor::IntOpaqueBlack => VkBorderColor::IntOpaqueBlack,
            BorderColor::IntOpaqueWhite => VkBorderColor::IntOpaqueWhite,
        }
    }
}

fn sampler_cache() -> &'static Mutex<HashMap<SamplerDescriptor, Arc<ChaosSampler>>> {
    static CACHE: OnceLock<Mutex<HashMap<SamplerDescriptor, Arc<ChaosSampler>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl ChaosSampler {
    /// Returns the cached sampler for `descriptor`, creating it on first use.
    pub fn get(render_context: &ChaosRenderContext, descriptor: SamplerDescriptor) -> Arc<Self> {
        let mut cache = sampler_cache().lock().unwrap();
        cache
            .entry(descriptor)
            .or_insert_with(|| {
                let create_info = descriptor.to_vulkano(render_context.mip_lod_bias());
                let vk_sampler = Sampler::new(render_context.device(), create_info.clone())
                    .expect("failed to create sampler");
                Arc::new(Self {
                    vk_sampler,
                    vk_sampler_create_info: create_info,
                })
            })
            .clone()
    }

    pub fn linear_clamp(render_context: &ChaosRenderContext) -> Arc<Self> {
        Self::get(render_context, SamplerDescriptor::LINEAR_CLAMP)
    }

    pub fn nearest_clamp(render_context: &ChaosRenderContext) -> Arc<Self> {
        Self::get(render_context, SamplerDescriptor::NEAREST_CLAMP)
    }

    pub fn linear_repeat(render_context: &ChaosRenderContext) -> Arc<Self> {
        Self::get(render_context, SamplerDescriptor::LINEAR_REPEAT)
    }
}

// `ChaosSampler::get`/`linear_clamp`/etc. need a real `ChaosRenderContext` (device, swapchain)
// to call `Sampler::new`, so they're exercised by integration/example runs rather than here;
// these tests cover the backend-agnostic descriptor logic that doesn't need a Vulkan device.
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn lod_clamp_none_matches_vulkano() {
        assert_eq!(LOD_CLAMP_NONE, vulkano::image::sampler::LOD_CLAMP_NONE);
    }

    #[test]
    fn filter_mode_maps_to_vulkano() {
        assert_eq!(VkFilter::from(FilterMode::Nearest), VkFilter::Nearest);
        assert_eq!(VkFilter::from(FilterMode::Linear), VkFilter::Linear);
    }

    #[test]
    fn mipmap_mode_maps_to_vulkano() {
        assert_eq!(
            SamplerMipmapMode::from(MipmapMode::Nearest),
            SamplerMipmapMode::Nearest
        );
        assert_eq!(
            SamplerMipmapMode::from(MipmapMode::Linear),
            SamplerMipmapMode::Linear
        );
    }

    #[test]
    fn address_mode_maps_to_vulkano() {
        assert_eq!(
            SamplerAddressMode::from(AddressMode::ClampToEdge),
            SamplerAddressMode::ClampToEdge
        );
        assert_eq!(
            SamplerAddressMode::from(AddressMode::Repeat),
            SamplerAddressMode::Repeat
        );
        assert_eq!(
            SamplerAddressMode::from(AddressMode::MirroredRepeat),
            SamplerAddressMode::MirroredRepeat
        );
        assert_eq!(
            SamplerAddressMode::from(AddressMode::ClampToBorder),
            SamplerAddressMode::ClampToBorder
        );
    }

    #[test]
    fn compare_op_maps_to_vulkano() {
        assert_eq!(VkCompareOp::from(CompareOp::Never), VkCompareOp::Never);
        assert_eq!(VkCompareOp::from(CompareOp::Less), VkCompareOp::Less);
        assert_eq!(VkCompareOp::from(CompareOp::Equal), VkCompareOp::Equal);
        assert_eq!(
            VkCompareOp::from(CompareOp::LessOrEqual),
            VkCompareOp::LessOrEqual
        );
        assert_eq!(VkCompareOp::from(CompareOp::Greater), VkCompareOp::Greater);
        assert_eq!(
            VkCompareOp::from(CompareOp::NotEqual),
            VkCompareOp::NotEqual
        );
        assert_eq!(
            VkCompareOp::from(CompareOp::GreaterOrEqual),
            VkCompareOp::GreaterOrEqual
        );
        assert_eq!(VkCompareOp::from(CompareOp::Always), VkCompareOp::Always);
    }

    #[test]
    fn border_color_maps_to_vulkano() {
        assert_eq!(
            VkBorderColor::from(BorderColor::FloatTransparentBlack),
            VkBorderColor::FloatTransparentBlack
        );
        assert_eq!(
            VkBorderColor::from(BorderColor::FloatOpaqueBlack),
            VkBorderColor::FloatOpaqueBlack
        );
        assert_eq!(
            VkBorderColor::from(BorderColor::FloatOpaqueWhite),
            VkBorderColor::FloatOpaqueWhite
        );
        assert_eq!(
            VkBorderColor::from(BorderColor::IntTransparentBlack),
            VkBorderColor::IntTransparentBlack
        );
        assert_eq!(
            VkBorderColor::from(BorderColor::IntOpaqueBlack),
            VkBorderColor::IntOpaqueBlack
        );
        assert_eq!(
            VkBorderColor::from(BorderColor::IntOpaqueWhite),
            VkBorderColor::IntOpaqueWhite
        );
    }

    #[test]
    fn presets_have_expected_shape() {
        assert_eq!(
            SamplerDescriptor::LINEAR_CLAMP.mag_filter,
            FilterMode::Linear
        );
        assert_eq!(
            SamplerDescriptor::LINEAR_CLAMP.mipmap_mode,
            MipmapMode::Linear
        );
        assert_eq!(
            SamplerDescriptor::LINEAR_CLAMP.address_mode,
            [AddressMode::ClampToEdge; 3]
        );

        assert_eq!(
            SamplerDescriptor::NEAREST_CLAMP.mag_filter,
            FilterMode::Nearest
        );
        assert_eq!(
            SamplerDescriptor::NEAREST_CLAMP.mipmap_mode,
            MipmapMode::Nearest
        );
        assert_eq!(
            SamplerDescriptor::NEAREST_CLAMP.address_mode,
            [AddressMode::ClampToEdge; 3]
        );

        assert_eq!(
            SamplerDescriptor::LINEAR_REPEAT.mag_filter,
            FilterMode::Linear
        );
        assert_eq!(
            SamplerDescriptor::LINEAR_REPEAT.address_mode,
            [AddressMode::Repeat; 3]
        );
    }

    #[test]
    fn descriptor_equality_compares_every_field() {
        let base = SamplerDescriptor::LINEAR_CLAMP;
        assert_eq!(base, base);

        let different_filter = SamplerDescriptor {
            mag_filter: FilterMode::Nearest,
            ..base
        };
        assert_ne!(base, different_filter);

        let different_address_mode = SamplerDescriptor {
            address_mode: [AddressMode::Repeat; 3],
            ..base
        };
        assert_ne!(base, different_address_mode);

        let different_lod = SamplerDescriptor {
            max_lod: LOD_CLAMP_NONE,
            ..base
        };
        assert_ne!(base, different_lod);

        let different_anisotropy = SamplerDescriptor {
            max_anisotropy: Some(16.0),
            ..base
        };
        assert_ne!(base, different_anisotropy);

        let different_compare = SamplerDescriptor {
            compare: Some(CompareOp::LessOrEqual),
            ..base
        };
        assert_ne!(base, different_compare);

        let different_border_color = SamplerDescriptor {
            border_color: BorderColor::IntOpaqueWhite,
            ..base
        };
        assert_ne!(base, different_border_color);
    }

    #[test]
    fn equal_descriptors_hash_equal() {
        let a = SamplerDescriptor {
            max_anisotropy: Some(4.0),
            min_lod: 0.25,
            ..SamplerDescriptor::LINEAR_REPEAT
        };
        let b = a;

        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    #[test]
    fn to_vulkano_applies_context_mip_lod_bias_and_descriptor_shape() {
        let descriptor = SamplerDescriptor {
            max_anisotropy: Some(8.0),
            min_lod: 1.0,
            max_lod: LOD_CLAMP_NONE,
            compare: Some(CompareOp::Less),
            border_color: BorderColor::IntOpaqueWhite,
            ..SamplerDescriptor::LINEAR_REPEAT
        };

        let create_info = descriptor.to_vulkano(0.5);

        assert_eq!(create_info.mag_filter, VkFilter::Linear);
        assert_eq!(create_info.min_filter, VkFilter::Linear);
        assert_eq!(create_info.mipmap_mode, SamplerMipmapMode::Linear);
        assert_eq!(create_info.address_mode, [SamplerAddressMode::Repeat; 3]);
        assert_eq!(create_info.mip_lod_bias, 0.5);
        assert_eq!(create_info.anisotropy, Some(8.0));
        assert_eq!(create_info.compare, Some(VkCompareOp::Less));
        assert_eq!(create_info.lod, 1.0..=LOD_CLAMP_NONE);
        assert_eq!(create_info.border_color, VkBorderColor::IntOpaqueWhite);
    }

    #[test]
    fn to_vulkano_uses_context_bias_even_for_zero_bias_presets() {
        let create_info = SamplerDescriptor::NEAREST_CLAMP.to_vulkano(-0.5);
        assert_eq!(create_info.mip_lod_bias, -0.5);
    }
}
