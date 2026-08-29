pub type NodeId = u32;
pub type FontId = u32;
pub type TextureId = u32;

pub mod assets;
pub mod ast;
pub mod colors;
pub mod default_font;
pub mod events;
pub mod glyph_atlas;
pub mod layout;
pub mod paint;
pub mod runtime;
pub mod selector;
pub mod style;
pub mod system;
