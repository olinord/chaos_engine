use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fontdue::Font;
use vulkano::pipeline::graphics::color_blend::{
    AttachmentBlend, ColorBlendAttachmentState, ColorBlendState,
};
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};

use crate::{
    BufferContents, Vertex,
    math::{Vec2, matrix::Mat4},
    rendering::{
        buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage},
        draw_command::{ChaosDrawCommand, ChaosDrawQueue, ChaosRenderPhase},
        effect::ChaosEffect,
        effect_factory::{EffectFactory, EffectUsage},
        renderer::ChaosRenderContext,
    },
    ui::{
        FontId, NodeId, TextureId,
        colors::Color,
        glyph_atlas::GlyphAtlas,
        layout::{LayoutBox, UiViewport},
        runtime::{UiNode, UiTag, UiTree},
        style::{Display, Overflow, TextAlign},
    },
};

#[derive(Clone, Debug)]
pub enum PaintCommand {
    Rect {
        node_id: NodeId,
        bounds: LayoutBox,
        bg: Color,
        border_color: Color,
        border_width: f32,
        corner_radius: f32,
        clip: Option<LayoutBox>,
    },
    Image {
        bounds: LayoutBox,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        texture: TextureId,
        tint: Color,
        clip: Option<LayoutBox>,
    },
    Text {
        bounds: LayoutBox,
        content: String,
        font: FontId,
        size: f32,
        color: Color,
        align: TextAlign,
        clip: Option<LayoutBox>,
    },
    ClipPush(LayoutBox),
    ClipPop,
}

#[derive(Clone, Debug, Default)]
pub struct PaintList {
    pub commands: Vec<PaintCommand>,
}

#[derive(Clone, Debug, Default)]
pub struct UiPainter;

const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

impl UiPainter {
    /// Pre-order walk over `tree` producing a flat `PaintList`. Skips nodes
    /// with `display: none` or `visibility: hidden` (whole subtree). Emits a
    /// `ClipPush`/`ClipPop` pair around subtrees whose node has
    /// `overflow: hidden`, so downstream backends can scissor or discard.
    pub fn build_paint_list(tree: &UiTree) -> PaintList {
        let mut list = PaintList::default();
        if tree.nodes.contains_key(&tree.root) {
            let mut clip_stack: Vec<LayoutBox> = Vec::new();
            emit_node(tree, tree.root, &mut clip_stack, &mut list);
        }
        list
    }
}

fn emit_node(tree: &UiTree, id: NodeId, clip_stack: &mut Vec<LayoutBox>, list: &mut PaintList) {
    let Some(node) = tree.nodes.get(&id) else {
        return;
    };
    if !is_visible(node) {
        return;
    }

    // Text leaf nodes get a dedicated Text command; no Rect, no recursion.
    if let UiTag::Text(ref content) = node.tag {
        list.commands.push(PaintCommand::Text {
            bounds: node.layout,
            content: content.clone(),
            font: 0,
            size: node.computed_style.font_size,
            color: node.computed_style.text_color,
            align: node.computed_style.text_align,
            clip: clip_stack.last().copied(),
        });
        return;
    }

    let active_clip = clip_stack.last().copied();
    let pushed = matches!(node.computed_style.overflow, Overflow::Hidden);
    if pushed {
        let effective = match active_clip {
            Some(prev) => intersect(prev, node.layout),
            None => node.layout,
        };
        clip_stack.push(effective);
        list.commands.push(PaintCommand::ClipPush(effective));
    }

    let clip_for_rect = clip_stack.last().copied();
    list.commands.push(PaintCommand::Rect {
        node_id: id,
        bounds: node.layout,
        bg: node.computed_style.background_color.unwrap_or(TRANSPARENT),
        border_color: node.computed_style.border_color.unwrap_or(TRANSPARENT),
        border_width: node.computed_style.border_width,
        corner_radius: node.computed_style.border_radius,
        clip: clip_for_rect,
    });

    let mut ordered = node.children.clone();
    // Stable sort by z-index so equal siblings keep document order.
    ordered.sort_by_key(|c| {
        tree.nodes
            .get(c)
            .map(|n| n.computed_style.z_index)
            .unwrap_or(0)
    });

    for child in ordered {
        emit_node(tree, child, clip_stack, list);
    }

    if pushed {
        clip_stack.pop();
        list.commands.push(PaintCommand::ClipPop);
    }
}

fn is_visible(node: &UiNode) -> bool {
    node.computed_style.display != Display::None && node.computed_style.visibility
}

fn intersect(a: LayoutBox, b: LayoutBox) -> LayoutBox {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    let width = (x1 - x0).max(0.0);
    let height = (y1 - y0).max(0.0);
    LayoutBox {
        x: x0,
        y: y0,
        width,
        height,
        content_x: x0,
        content_y: y0,
        content_width: width,
        content_height: height,
    }
}

pub trait UiRenderBackend {
    fn upload_resources(
        &mut self,
        _ctx: &Arc<ChaosRenderContext>,
        _paint: &PaintList,
        _viewport: UiViewport,
    ) -> Result<(), String> {
        Ok(())
    }
    fn submit_paint_list(
        &mut self,
        _paint: &PaintList,
        _draw_queue: &mut ChaosDrawQueue,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[derive(BufferContents, Vertex, Copy, Clone)]
#[repr(C)]
pub struct UiRectVertex {
    #[format(R32G32_SFLOAT)]
    pub corner: Vec2,
}

/// GPU-facing per-rect instance data. Field layout matches the `RectInstance`
/// struct declared in `res/internal/shaders/ui_rect.vert` under std430 rules
/// (five vec4s = 80 bytes, no padding).
#[derive(BufferContents, Copy, Clone, Debug)]
#[repr(C)]
pub struct UiRectInstance {
    pub rect: [f32; 4],
    pub bg: [f32; 4],
    pub border_color: [f32; 4],
    pub clip_rect: [f32; 4],
    pub params: [f32; 4],
}

/// GPU-facing per-glyph instance data. Matches `GlyphInstance` in `ui_text.vert`.
#[derive(BufferContents, Copy, Clone, Debug)]
#[repr(C)]
pub struct UiGlyphInstance {
    /// x, y, width, height in logical pixels.
    pub pos_size: [f32; 4],
    /// uv_min.x, uv_min.y, uv_max.x, uv_max.y.
    pub uv: [f32; 4],
    /// RGBA text colour.
    pub color: [f32; 4],
    /// Clip rect: xmin, ymin, xmax, ymax.
    pub clip_rect: [f32; 4],
}

/// GPU-facing projection uniform.
#[derive(BufferContents, Copy, Clone)]
#[repr(C)]
struct UiProjectionUniform {
    view_proj: Mat4,
}

/// Projection uniform for the text pipeline. Includes atlas dimensions so the
/// fragment shader can convert UV to texel coordinates.
#[derive(BufferContents, Copy, Clone)]
#[repr(C)]
struct UiTextProjection {
    view_proj: Mat4,
    atlas_width: u32,
    atlas_height: u32,
    _pad0: u32,
    _pad1: u32,
}

/// Path key registered with `ChaosEngine::add_directory` for the engine's UI
/// shaders. Clients that want the default backend to work must add this alias
/// pointing at the compiled `ui_rect.vert`/`.frag`.
pub const DEFAULT_UI_SHADER_ALIAS: &str = "internal_shaders";

/// Vulkan-backed implementation of [`UiRenderBackend`]. Draws every rect
/// command as an SDF-shaded rounded rectangle instance in a single draw call
/// per paint list, and text glyphs via a storage-buffer atlas in a second call.
pub struct VulkanUiBackend {
    effect: Arc<Mutex<ChaosEffect>>,
    quad_buffer: Arc<Mutex<ChaosBuffer>>,
    instance_count: u32,
    shader_alias: String,
    // text pipeline (optional — only present when enable_text() has been called)
    text_effect: Option<Arc<Mutex<ChaosEffect>>>,
    glyph_instance_count: u32,
    atlas: GlyphAtlas,
    fonts: HashMap<FontId, Font>,
}

impl VulkanUiBackend {
    /// Uses [`DEFAULT_UI_SHADER_ALIAS`] as the shader path prefix.
    pub fn new(ctx: &Arc<ChaosRenderContext>) -> Result<Self, String> {
        Self::with_shader_alias(ctx, DEFAULT_UI_SHADER_ALIAS)
    }

    pub fn with_shader_alias(ctx: &Arc<ChaosRenderContext>, alias: &str) -> Result<Self, String> {
        let path: std::path::PathBuf = format!("{alias}:/ui_rect").into();
        let usage = EffectUsage::new(path)
            .with_color_blend_state(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend::alpha()),
                    ..Default::default()
                },
            ))
            .with_rasterization_state(RasterizationState {
                cull_mode: CullMode::None,
                ..RasterizationState::default()
            });

        let effect = EffectFactory::instance()
            .get_effect::<UiRectVertex>(&usage, ctx)
            .map_err(|e| format!("failed to build ui_rect effect: {e}"))?;

        let mut quad = ChaosBuffer::new(
            "ui-rect-quad".into(),
            ChaosBufferUsage::VertexBuffer,
            ChaosBufferMemoryType::PreferDevice,
            ctx.clone(),
        );
        quad.set_data_from_vec(unit_quad_vertices())?;

        Ok(Self {
            effect: Arc::new(Mutex::new(effect)),
            quad_buffer: Arc::new(Mutex::new(quad)),
            instance_count: 0,
            shader_alias: alias.to_string(),
            text_effect: None,
            glyph_instance_count: 0,
            atlas: GlyphAtlas::default(),
            fonts: HashMap::new(),
        })
    }

    pub fn shader_alias(&self) -> &str {
        &self.shader_alias
    }

    /// Compile the text pipeline (`ui_text.vert/.frag`) and enable text
    /// rendering. Call once after construction, before the first frame.
    pub fn enable_text(&mut self, ctx: &Arc<ChaosRenderContext>) -> Result<(), String> {
        let alias = &self.shader_alias;
        let path: std::path::PathBuf = format!("{alias}:/ui_text").into();
        let usage = EffectUsage::new(path)
            .with_color_blend_state(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend::alpha()),
                    ..Default::default()
                },
            ))
            .with_rasterization_state(RasterizationState {
                cull_mode: CullMode::None,
                ..RasterizationState::default()
            });

        let effect = EffectFactory::instance()
            .get_effect::<UiRectVertex>(&usage, ctx)
            .map_err(|e| format!("failed to build ui_text effect: {e}"))?;

        self.text_effect = Some(Arc::new(Mutex::new(effect)));
        Ok(())
    }

    /// Register a fontdue font for glyph rasterization.
    pub fn add_font(&mut self, id: FontId, font: Font) {
        self.fonts.insert(id, font);
    }
}

impl UiRenderBackend for VulkanUiBackend {
    fn upload_resources(
        &mut self,
        _ctx: &Arc<ChaosRenderContext>,
        paint: &PaintList,
        viewport: UiViewport,
    ) -> Result<(), String> {
        // --- rect pipeline ---
        let instances = flatten_rect_instances(paint);
        self.instance_count = instances.len() as u32;

        let mut effect = self
            .effect
            .lock()
            .map_err(|_| "failed to lock ui effect".to_string())?;

        effect.set_uniform_data(0, 0, projection_uniform_for(viewport))?;
        if !instances.is_empty() {
            effect.set_storage_data_vec(0, 1, instances)?;
        } else {
            // Vulkano rejects empty storage buffers; keep a placeholder.
            effect.set_storage_data_vec(
                0,
                1,
                vec![UiRectInstance {
                    rect: [0.0; 4],
                    bg: [0.0; 4],
                    border_color: [0.0; 4],
                    clip_rect: [0.0; 4],
                    params: [0.0; 4],
                }],
            )?;
        }
        drop(effect);

        // --- text pipeline ---
        let Some(ref text_effect_arc) = self.text_effect else {
            return Ok(());
        };
        let glyph_instances = build_glyph_instances(paint, &self.fonts, &mut self.atlas);
        self.glyph_instance_count = glyph_instances.len() as u32;

        let mut text_effect = text_effect_arc
            .lock()
            .map_err(|_| "failed to lock ui text effect".to_string())?;

        text_effect.set_uniform_data(
            0,
            0,
            UiTextProjection {
                view_proj: projection_uniform_for(viewport).view_proj,
                atlas_width: self.atlas.width,
                atlas_height: self.atlas.height,
                _pad0: 0,
                _pad1: 0,
            },
        )?;

        let placeholder_glyph = UiGlyphInstance {
            pos_size: [0.0; 4],
            uv: [0.0; 4],
            color: [0.0; 4],
            clip_rect: [0.0; 4],
        };
        if !glyph_instances.is_empty() {
            text_effect.set_storage_data_vec(0, 1, glyph_instances)?;
        } else {
            text_effect.set_storage_data_vec(0, 1, vec![placeholder_glyph])?;
        }

        // Upload atlas as packed u32 storage buffer when contents changed.
        if self.atlas.dirty {
            let packed = self.atlas.pack_as_u32();
            if !packed.is_empty() {
                text_effect.set_storage_data_vec(0, 2, packed)?;
            } else {
                text_effect.set_storage_data_vec(0, 2, vec![0u32])?;
            }
            self.atlas.dirty = false;
        }

        Ok(())
    }

    fn submit_paint_list(
        &mut self,
        _paint: &PaintList,
        draw_queue: &mut ChaosDrawQueue,
    ) -> Result<(), String> {
        // Rect draw call.
        if self.instance_count > 0 {
            let effect = self.effect.clone();
            let quad_buffer = self.quad_buffer.clone();
            let instance_count = self.instance_count;
            let effect_hash = effect
                .lock()
                .map_err(|_| "failed to lock ui effect".to_string())?
                .hash;

            draw_queue.push(ChaosDrawCommand {
                render_phase: ChaosRenderPhase::Transparent,
                effect_hash,
                draw_function: Box::new(move |command_buffer| {
                    let effect_guard = effect.lock().map_err(|_| "failed to lock ui effect")?;
                    let quad_guard = quad_buffer
                        .lock()
                        .map_err(|_| "failed to lock ui quad buffer")?;
                    effect_guard
                        .bind_descriptor_sets(command_buffer)
                        .map_err(|_| "failed to bind ui descriptor sets")?;
                    command_buffer
                        .bind_pipeline_graphics(effect_guard.pipeline())
                        .map_err(|_| "failed to bind ui pipeline")?;
                    quad_guard
                        .bind_as_vertex_buffer(0, command_buffer)
                        .map_err(|_| "failed to bind ui quad vertex buffer")?;
                    unsafe {
                        command_buffer
                            .draw(6, instance_count, 0, 0)
                            .map_err(|_| "failed to submit ui draw call")?;
                    }
                    Ok(())
                }),
            });
        }

        // Text draw call (submitted after rects so text renders on top).
        if self.glyph_instance_count > 0 {
            if let Some(ref text_effect_arc) = self.text_effect {
                let text_effect = text_effect_arc.clone();
                let quad_buffer = self.quad_buffer.clone();
                let glyph_instance_count = self.glyph_instance_count;
                let text_hash = text_effect
                    .lock()
                    .map_err(|_| "failed to lock ui text effect".to_string())?
                    .hash;

                draw_queue.push(ChaosDrawCommand {
                    render_phase: ChaosRenderPhase::UiText,
                    effect_hash: text_hash,
                    draw_function: Box::new(move |command_buffer| {
                        let effect_guard = text_effect
                            .lock()
                            .map_err(|_| "failed to lock ui text effect")?;
                        let quad_guard = quad_buffer
                            .lock()
                            .map_err(|_| "failed to lock ui text quad buffer")?;
                        effect_guard
                            .bind_descriptor_sets(command_buffer)
                            .map_err(|_| "failed to bind ui text descriptor sets")?;
                        command_buffer
                            .bind_pipeline_graphics(effect_guard.pipeline())
                            .map_err(|_| "failed to bind ui text pipeline")?;
                        quad_guard
                            .bind_as_vertex_buffer(0, command_buffer)
                            .map_err(|_| "failed to bind ui text quad vertex buffer")?;
                        unsafe {
                            command_buffer
                                .draw(6, glyph_instance_count, 0, 0)
                                .map_err(|_| "failed to submit ui text draw call")?;
                        }
                        Ok(())
                    }),
                });
            }
        }

        Ok(())
    }
}

fn unit_quad_vertices() -> Vec<UiRectVertex> {
    let c = |x: f32, y: f32| UiRectVertex {
        corner: Vec2::new(x, y),
    };
    vec![
        c(0.0, 0.0),
        c(1.0, 0.0),
        c(1.0, 1.0),
        c(0.0, 0.0),
        c(1.0, 1.0),
        c(0.0, 1.0),
    ]
    .into()
}

fn projection_uniform_for(viewport: UiViewport) -> UiProjectionUniform {
    let scale = if viewport.scale_factor > 0.0 {
        viewport.scale_factor
    } else {
        1.0
    };
    let w = (viewport.width / scale).max(1.0);
    let h = (viewport.height / scale).max(1.0);
    // Top-left origin: bottom=h, top=0. The renderer uses a negative-height
    // viewport, so this OpenGL-style ortho ends up correct on screen.
    let view_proj = Mat4::orthographic_projection(0.0, w, h, 0.0, -1.0, 1.0);
    UiProjectionUniform { view_proj }
}

fn flatten_rect_instances(paint: &PaintList) -> Vec<UiRectInstance> {
    let mut out = Vec::with_capacity(paint.commands.len());
    for cmd in &paint.commands {
        if let PaintCommand::Rect {
            node_id: _,
            bounds,
            bg,
            border_color,
            border_width,
            corner_radius,
            clip,
        } = cmd
        {
            let clip_rect = match clip {
                Some(c) => [c.x, c.y, c.x + c.width, c.y + c.height],
                None => [
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::INFINITY,
                ],
            };
            out.push(UiRectInstance {
                rect: [bounds.x, bounds.y, bounds.width, bounds.height],
                bg: [bg.r, bg.g, bg.b, bg.a],
                border_color: [
                    border_color.r,
                    border_color.g,
                    border_color.b,
                    border_color.a,
                ],
                clip_rect,
                params: [*border_width, *corner_radius, 0.0, 0.0],
            });
        }
    }
    out
}

/// Walk `paint` for `Text` commands, rasterize each glyph into `atlas` (if not
/// already cached), and return a flat list of per-glyph quad instances.
fn build_glyph_instances(
    paint: &PaintList,
    fonts: &HashMap<FontId, Font>,
    atlas: &mut GlyphAtlas,
) -> Vec<UiGlyphInstance> {
    let mut out = Vec::new();
    for cmd in &paint.commands {
        let PaintCommand::Text {
            bounds,
            content,
            font: font_id,
            size,
            color,
            clip,
            ..
        } = cmd
        else {
            continue;
        };
        let Some(font) = fonts.get(font_id) else {
            continue;
        };
        let clip_rect = match clip {
            Some(c) => [c.x, c.y, c.x + c.width, c.y + c.height],
            None => [
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::INFINITY,
            ],
        };
        let baseline_y = bounds.content_y + *size;
        let mut cursor_x = bounds.content_x;
        for ch in content.chars() {
            let entry = atlas.get_or_rasterize(*font_id, font, *size, ch);
            if entry.width > 0 && entry.height > 0 {
                let draw_x = cursor_x + entry.bearing_x;
                let draw_y = baseline_y - entry.bearing_y;
                out.push(UiGlyphInstance {
                    pos_size: [draw_x, draw_y, entry.width as f32, entry.height as f32],
                    uv: [
                        entry.uv_min[0],
                        entry.uv_min[1],
                        entry.uv_max[0],
                        entry.uv_max[1],
                    ],
                    color: [color.r, color.g, color.b, color.a],
                    clip_rect,
                });
            }
            cursor_x += entry.advance_x;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::runtime::{DefaultUiTemplateInstantiator, UiTemplate, UiTemplateInstantiator};

    fn instantiate(markup: &str, css: &str) -> UiTree {
        let template = UiTemplate::from_src(markup, css).unwrap();
        let mut tree = DefaultUiTemplateInstantiator.instantiate(&template);
        tree.apply_style(&template.stylesheet).unwrap();
        tree
    }

    fn set_layout(tree: &mut UiTree, id: NodeId, x: f32, y: f32, w: f32, h: f32) {
        let n = tree.nodes.get_mut(&id).unwrap();
        n.layout = LayoutBox {
            x,
            y,
            width: w,
            height: h,
            content_x: x,
            content_y: y,
            content_width: w,
            content_height: h,
        };
    }

    #[test]
    fn nested_tree_emits_expected_command_sequence() {
        let markup = r#"
            <ui id="root">
                <ui id="a"/>
                <ui id="b"/>
            </ui>
        "#;
        let css = r#"
            #root { background-color: rgb(10, 20, 30); }
            #a { background-color: rgb(40, 50, 60); }
            #b { background-color: rgb(70, 80, 90); }
        "#;
        let tree = instantiate(markup, css);
        let list = UiPainter::build_paint_list(&tree);
        assert_eq!(list.commands.len(), 3);
        for cmd in &list.commands {
            assert!(matches!(cmd, PaintCommand::Rect { .. }));
        }
    }

    #[test]
    fn display_none_skips_node_and_subtree() {
        let markup = r#"
            <ui id="root">
                <ui id="hidden">
                    <ui id="grandchild"/>
                </ui>
                <ui id="visible"/>
            </ui>
        "#;
        let css = r#"
            #hidden { display: none; }
        "#;
        let tree = instantiate(markup, css);
        let list = UiPainter::build_paint_list(&tree);
        assert_eq!(list.commands.len(), 2);
        for cmd in &list.commands {
            assert!(matches!(cmd, PaintCommand::Rect { .. }));
        }
    }

    #[test]
    fn overflow_hidden_wraps_subtree_in_clip_push_pop() {
        let markup = r#"
            <ui id="root">
                <ui id="clipper">
                    <ui id="child"/>
                </ui>
            </ui>
        "#;
        let css = r#"
            #clipper { overflow: hidden; }
        "#;
        let mut tree = instantiate(markup, css);
        let root = tree.root;
        let clipper = tree.nodes.get(&root).unwrap().children[0];
        let child = tree.nodes.get(&clipper).unwrap().children[0];
        set_layout(&mut tree, root, 0.0, 0.0, 200.0, 200.0);
        set_layout(&mut tree, clipper, 10.0, 20.0, 50.0, 60.0);
        set_layout(&mut tree, child, 15.0, 25.0, 30.0, 30.0);

        let list = UiPainter::build_paint_list(&tree);
        // Expected: Rect(root), ClipPush, Rect(clipper), Rect(child), ClipPop
        assert_eq!(list.commands.len(), 5);
        assert!(matches!(list.commands[0], PaintCommand::Rect { .. }));
        match &list.commands[1] {
            PaintCommand::ClipPush(b) => {
                assert!((b.x - 10.0).abs() < 0.001);
                assert!((b.y - 20.0).abs() < 0.001);
                assert!((b.width - 50.0).abs() < 0.001);
                assert!((b.height - 60.0).abs() < 0.001);
            }
            other => panic!("expected ClipPush, got {:?}", other),
        }
        match &list.commands[2] {
            PaintCommand::Rect { clip: Some(c), .. } => {
                assert!((c.width - 50.0).abs() < 0.001);
            }
            other => panic!("expected clipped Rect for clipper, got {:?}", other),
        }
        match &list.commands[3] {
            PaintCommand::Rect { clip: Some(_), .. } => {}
            other => panic!("expected clipped Rect for child, got {:?}", other),
        }
        assert!(matches!(list.commands[4], PaintCommand::ClipPop));
    }

    #[test]
    fn children_sorted_by_z_index_stable() {
        let markup = r#"
            <ui id="root">
                <ui id="a"/>
                <ui id="b"/>
                <ui id="c"/>
            </ui>
        "#;
        let css = r#"
            #a { z-index: 5; }
            #b { z-index: 1; }
            #c { z-index: 5; }
        "#;
        let mut tree = instantiate(markup, css);
        let root = tree.root;
        let kids = tree.nodes.get(&root).unwrap().children.clone();
        for (i, id) in kids.iter().enumerate() {
            set_layout(&mut tree, *id, i as f32 * 10.0, 0.0, 5.0, 5.0);
        }
        let (a_x, b_x, c_x) = (0.0_f32, 10.0_f32, 20.0_f32);

        let list = UiPainter::build_paint_list(&tree);
        // Root Rect first, then children sorted: b (z=1), a (z=5), c (z=5).
        let xs: Vec<f32> = list
            .commands
            .iter()
            .skip(1)
            .filter_map(|c| match c {
                PaintCommand::Rect { bounds, .. } => Some(bounds.x),
                _ => None,
            })
            .collect();
        assert_eq!(xs, vec![b_x, a_x, c_x]);
    }

    #[test]
    fn text_inline_in_ui_emits_text_command() {
        // Text inline content in a <ui> element should produce a child Text node
        // and emit PaintCommand::Text (not a Rect).
        let markup = r#"<ui id="root">Hello</ui>"#;
        let tree = instantiate(markup, "");
        let list = UiPainter::build_paint_list(&tree);
        let text_cmds: Vec<_> = list
            .commands
            .iter()
            .filter(|c| matches!(c, PaintCommand::Text { .. }))
            .collect();
        assert_eq!(text_cmds.len(), 1, "expected one Text command");
        match &text_cmds[0] {
            PaintCommand::Text { content, .. } => {
                assert_eq!(content, "Hello");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn text_element_also_emits_text_command() {
        let markup = r#"<ui id="root"><text>World</text></ui>"#;
        let tree = instantiate(markup, "");
        let list = UiPainter::build_paint_list(&tree);
        let text_cmds: usize = list
            .commands
            .iter()
            .filter(|c| matches!(c, PaintCommand::Text { .. }))
            .count();
        assert_eq!(
            text_cmds, 1,
            "expected one Text command from <text> element"
        );
    }
}
