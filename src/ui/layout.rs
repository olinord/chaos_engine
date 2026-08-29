use std::collections::HashMap;

use fontdue::Font;
use taffy::{
    AvailableSpace, NodeId as TaffyNodeId, Point, Rect, Size, Style, TaffyTree,
    prelude::{auto, length, percent, zero},
    style::{Dimension, LengthPercentage, LengthPercentageAuto, Overflow as TaffyOverflow},
};

use crate::ui::{
    FontId, NodeId,
    runtime::{DirtyFlag, UiTag, UiTree},
    style::{AlignItems, ComputedStyle, Display, FlexDirection, JustifyContent, Length, Overflow},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub content_x: f32,
    pub content_y: f32,
    pub content_width: f32,
    pub content_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct UiViewport {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

impl Default for UiViewport {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            scale_factor: 1.0,
        }
    }
}

impl UiViewport {
    fn logical_size(&self) -> (f32, f32) {
        let s = if self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        };
        (self.width / s, self.height / s)
    }
}

pub struct UiLayoutEngine {
    taffy: TaffyTree<NodeId>,
    ui_to_taffy: HashMap<NodeId, TaffyNodeId>,
    taffy_to_ui: HashMap<TaffyNodeId, NodeId>,
    root_taffy: Option<TaffyNodeId>,
    fonts: HashMap<FontId, Font>,
}

impl Default for UiLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl UiLayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            ui_to_taffy: HashMap::new(),
            taffy_to_ui: HashMap::new(),
            root_taffy: None,
            fonts: HashMap::new(),
        }
    }

    /// Register a fontdue font so text nodes can be measured during layout.
    pub fn add_font(&mut self, id: FontId, font: Font) {
        self.fonts.insert(id, font);
    }

    pub fn taffy_node_for(&self, ui: NodeId) -> Option<TaffyNodeId> {
        self.ui_to_taffy.get(&ui).copied()
    }

    pub fn ui_node_for(&self, taffy: TaffyNodeId) -> Option<NodeId> {
        self.taffy_to_ui.get(&taffy).copied()
    }

    /// Reconcile taffy tree structure with the UI tree.
    ///
    /// - Drains `pending_taffy_removals` and deletes those taffy nodes.
    /// - Creates taffy nodes for any UI node that doesn't have one yet
    ///   (freshly-instantiated templates hit this path).
    /// - For every `Structure`-dirty UI node, pushes its current child list
    ///   into taffy and clears the flag.
    pub fn sync_structure(&mut self, ui_tree: &mut UiTree) -> Result<(), String> {
        for ui_id in ui_tree.pending_taffy_removals.drain(..) {
            if let Some(taffy_id) = self.ui_to_taffy.remove(&ui_id) {
                self.taffy_to_ui.remove(&taffy_id);
                self.taffy
                    .remove(taffy_id)
                    .map_err(|e| format!("taffy remove failed: {:?}", e))?;
            }
        }

        let mut stack = vec![ui_tree.root];
        while let Some(id) = stack.pop() {
            let (children, parent) = match ui_tree.nodes.get(&id) {
                Some(node) => (node.children.clone(), node.parent),
                None => continue,
            };
            if !self.ui_to_taffy.contains_key(&id) {
                let taffy_id = self
                    .taffy
                    .new_leaf(Style::default())
                    .map_err(|e| format!("taffy new_leaf failed: {:?}", e))?;
                self.ui_to_taffy.insert(id, taffy_id);
                self.taffy_to_ui.insert(taffy_id, id);
                if let Some(n) = ui_tree.nodes.get_mut(&id) {
                    n.dirty.insert(DirtyFlag::Layout);
                }
                // A newly-added taffy node needs its parent's child list
                // resynced so it actually participates in layout.
                if let Some(parent_id) = parent {
                    if let Some(p) = ui_tree.nodes.get_mut(&parent_id) {
                        p.dirty.insert(DirtyFlag::Structure);
                    }
                }
            }
            for c in children {
                stack.push(c);
            }
        }

        if let Some(&taffy_root) = self.ui_to_taffy.get(&ui_tree.root) {
            self.root_taffy = Some(taffy_root);
        }

        let structure_dirty: Vec<NodeId> = ui_tree
            .nodes
            .iter()
            .filter(|(_, n)| n.dirty.contains(&DirtyFlag::Structure))
            .map(|(&id, _)| id)
            .collect();
        for id in structure_dirty {
            let Some(taffy_id) = self.ui_to_taffy.get(&id).copied() else {
                continue;
            };
            let children_ui: Vec<NodeId> = ui_tree
                .nodes
                .get(&id)
                .map(|n| n.children.clone())
                .unwrap_or_default();
            let children_taffy: Vec<TaffyNodeId> = children_ui
                .iter()
                .filter_map(|c| self.ui_to_taffy.get(c).copied())
                .collect();
            self.taffy
                .set_children(taffy_id, &children_taffy)
                .map_err(|e| format!("taffy set_children failed: {:?}", e))?;
            if let Some(n) = ui_tree.nodes.get_mut(&id) {
                n.dirty.remove(&DirtyFlag::Structure);
            }
        }

        Ok(())
    }

    /// Push updated taffy `Style` for every `Layout`-dirty node. The dirty
    /// flag is intentionally not cleared here — `compute_layout` clears it
    /// after writing the resolved `LayoutBox`.
    pub fn sync_style(&mut self, ui_tree: &UiTree) -> Result<(), String> {
        let dirty: Vec<NodeId> = ui_tree
            .nodes
            .iter()
            .filter(|(_, n)| n.dirty.contains(&DirtyFlag::Layout))
            .map(|(&id, _)| id)
            .collect();
        for id in dirty {
            let Some(taffy_id) = self.ui_to_taffy.get(&id).copied() else {
                continue;
            };
            let Some(node) = ui_tree.nodes.get(&id) else {
                continue;
            };
            let style = to_taffy_style(&node.computed_style);
            self.taffy
                .set_style(taffy_id, style)
                .map_err(|e| format!("taffy set_style failed: {:?}", e))?;
        }
        Ok(())
    }

    /// Runs `sync_style`, then Taffy's layout pass, then writes absolute
    /// `LayoutBox` values back to every UI node. Nodes whose box changed get
    /// `Paint` dirty.
    pub fn compute_layout(
        &mut self,
        ui_tree: &mut UiTree,
        viewport: UiViewport,
    ) -> Result<(), String> {
        self.sync_style(ui_tree)?;

        let Some(root) = self.root_taffy else {
            return Ok(());
        };
        let (log_w, log_h) = viewport.logical_size();
        let available = Size {
            width: AvailableSpace::Definite(log_w),
            height: AvailableSpace::Definite(log_h),
        };

        // Pre-collect text node data so the closure doesn't need to borrow ui_tree.
        let text_info: HashMap<TaffyNodeId, (String, FontId, f32)> = self
            .ui_to_taffy
            .iter()
            .filter_map(|(ui_id, taffy_id)| {
                let node = ui_tree.nodes.get(ui_id)?;
                match &node.tag {
                    UiTag::Text(content) if !content.is_empty() => Some((
                        *taffy_id,
                        (content.clone(), 0u32, node.computed_style.font_size),
                    )),
                    _ => None,
                }
            })
            .collect();

        let fonts = &self.fonts;
        self.taffy
            .compute_layout_with_measure(root, available, |_known, avail, taffy_id, _, _| {
                let Some((text, font_id, size_px)) = text_info.get(&taffy_id) else {
                    return Size::ZERO;
                };
                let width = fonts
                    .get(font_id)
                    .map(|f| measure_text_width(f, *size_px, text))
                    .unwrap_or_else(|| size_px * 0.6 * text.len() as f32);
                let max_w = match avail.width {
                    AvailableSpace::Definite(w) => w,
                    _ => f32::MAX,
                };
                Size {
                    width: width.min(max_w),
                    height: *size_px,
                }
            })
            .map_err(|e| format!("taffy compute_layout failed: {:?}", e))?;

        self.write_layout_boxes(ui_tree, ui_tree.root, 0.0, 0.0);
        Ok(())
    }

    fn write_layout_boxes(&self, ui_tree: &mut UiTree, id: NodeId, parent_x: f32, parent_y: f32) {
        let Some(taffy_id) = self.ui_to_taffy.get(&id).copied() else {
            return;
        };
        let Ok(layout) = self.taffy.layout(taffy_id) else {
            return;
        };
        let x = parent_x + layout.location.x;
        let y = parent_y + layout.location.y;
        let width = layout.size.width;
        let height = layout.size.height;
        let pad = layout.padding;
        let bor = layout.border;
        let content_x = x + pad.left + bor.left;
        let content_y = y + pad.top + bor.top;
        let content_width = (width - pad.left - pad.right - bor.left - bor.right).max(0.0);
        let content_height = (height - pad.top - pad.bottom - bor.top - bor.bottom).max(0.0);
        let new_box = LayoutBox {
            x,
            y,
            width,
            height,
            content_x,
            content_y,
            content_width,
            content_height,
        };

        let children: Vec<NodeId> = ui_tree
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        if let Some(n) = ui_tree.nodes.get_mut(&id) {
            if n.layout != new_box {
                n.dirty.insert(DirtyFlag::Paint);
            }
            n.layout = new_box;
            n.dirty.remove(&DirtyFlag::Layout);
        }

        for c in children {
            self.write_layout_boxes(ui_tree, c, new_box.x, new_box.y);
        }
    }
}

/// Sum the advance widths of all characters in `text` using fontdue metrics.
fn measure_text_width(font: &Font, size_px: f32, text: &str) -> f32 {
    text.chars()
        .map(|c| font.metrics(c, size_px).advance_width)
        .sum()
}

fn length_to_dimension(l: Length) -> Dimension {
    match l {
        Length::Px(v) => length(v),
        Length::Percent(v) => percent(v),
        Length::Auto => auto(),
    }
}

fn length_to_lp_auto(l: Length) -> LengthPercentageAuto {
    match l {
        Length::Px(v) => length(v),
        Length::Percent(v) => percent(v),
        Length::Auto => auto(),
    }
}

fn length_to_lp(l: Length) -> LengthPercentage {
    match l {
        Length::Px(v) => length(v),
        Length::Percent(v) => percent(v),
        Length::Auto => zero(),
    }
}

/// Map our `ComputedStyle` onto a `taffy::Style` suitable for the flex layout
/// algorithm.
pub fn to_taffy_style(style: &ComputedStyle) -> Style {
    let display = match style.display {
        Display::Flex => taffy::style::Display::Flex,
        Display::None => taffy::style::Display::None,
    };
    let flex_direction = match style.flex_direction {
        FlexDirection::Row => taffy::style::FlexDirection::Row,
        FlexDirection::Column => taffy::style::FlexDirection::Column,
    };
    let align_items = Some(match style.align_items {
        AlignItems::Start => taffy::style::AlignItems::FLEX_START,
        AlignItems::Center => taffy::style::AlignItems::CENTER,
        AlignItems::End => taffy::style::AlignItems::FLEX_END,
        AlignItems::Stretch => taffy::style::AlignItems::STRETCH,
    });
    let justify_content = Some(match style.justify_content {
        JustifyContent::Start => taffy::style::JustifyContent::FLEX_START,
        JustifyContent::Center => taffy::style::JustifyContent::CENTER,
        JustifyContent::End => taffy::style::JustifyContent::FLEX_END,
        JustifyContent::SpaceBetween => taffy::style::JustifyContent::SPACE_BETWEEN,
        JustifyContent::SpaceAround => taffy::style::JustifyContent::SPACE_AROUND,
    });
    let overflow_v = match style.overflow {
        Overflow::Visible => TaffyOverflow::Visible,
        Overflow::Hidden => TaffyOverflow::Hidden,
    };

    let [m_top, m_right, m_bottom, m_left] = style.margin;
    let [p_top, p_right, p_bottom, p_left] = style.padding;
    let border_lp: LengthPercentage = length(style.border_width);

    Style {
        display,
        size: Size {
            width: length_to_dimension(style.width),
            height: length_to_dimension(style.height),
        },
        min_size: Size {
            width: length_to_dimension(style.min_width),
            height: length_to_dimension(style.min_height),
        },
        max_size: Size {
            width: length_to_dimension(style.max_width),
            height: length_to_dimension(style.max_height),
        },
        margin: Rect {
            left: length_to_lp_auto(m_left),
            right: length_to_lp_auto(m_right),
            top: length_to_lp_auto(m_top),
            bottom: length_to_lp_auto(m_bottom),
        },
        padding: Rect {
            left: length_to_lp(p_left),
            right: length_to_lp(p_right),
            top: length_to_lp(p_top),
            bottom: length_to_lp(p_bottom),
        },
        border: Rect {
            left: border_lp,
            right: border_lp,
            top: border_lp,
            bottom: border_lp,
        },
        gap: Size {
            width: length_to_lp(style.gap),
            height: length_to_lp(style.gap),
        },
        flex_direction,
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        align_items,
        justify_content,
        overflow: Point {
            x: overflow_v,
            y: overflow_v,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::runtime::{DefaultUiTemplateInstantiator, UiTemplate, UiTemplateInstantiator};

    fn viewport(w: f32, h: f32) -> UiViewport {
        UiViewport {
            width: w,
            height: h,
            scale_factor: 1.0,
        }
    }

    fn instantiate(markup: &str, css: &str) -> UiTree {
        let template = UiTemplate::from_src(markup, css).unwrap();
        let mut tree = DefaultUiTemplateInstantiator.instantiate(&template);
        tree.apply_style(&template.stylesheet).unwrap();
        tree
    }

    #[test]
    fn flex_row_distributes_width_with_gap() {
        let markup = r#"
            <ui id="root">
                <ui class="child"/>
                <ui class="child"/>
                <ui class="child"/>
            </ui>
        "#;
        let css = r#"
            #root { display: flex; flex-direction: row; width: 300px; height: 100px; gap: 10px; }
            .child { flex-grow: 1; height: 100px; }
        "#;
        let mut tree = instantiate(markup, css);
        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();
        engine
            .compute_layout(&mut tree, viewport(400.0, 400.0))
            .unwrap();

        let root_children = tree.nodes.get(&tree.root).unwrap().children.clone();
        assert_eq!(root_children.len(), 3);
        let boxes: Vec<LayoutBox> = root_children
            .iter()
            .map(|c| tree.nodes.get(c).unwrap().layout)
            .collect();

        // Taffy rounds to whole pixels by default, so allow slight slack.
        let expected_w = (300.0 - 20.0) / 3.0;
        for b in &boxes {
            assert!(
                (b.width - expected_w).abs() < 1.0,
                "expected ~{}, got {}",
                expected_w,
                b.width
            );
            assert!((b.height - 100.0).abs() < 0.01);
        }
        // Total width used should equal 300px exactly (rounding balanced across siblings).
        let total_child_and_gap: f32 = boxes.iter().map(|b| b.width).sum::<f32>() + 20.0;
        assert!((total_child_and_gap - 300.0).abs() < 0.5);
    }

    #[test]
    fn padding_shrinks_content_box() {
        let markup = r#"<ui id="root"/>"#;
        let css = r#"
            #root { width: 200px; height: 100px; padding: 10px, 20px, 30px, 40px; }
        "#;
        let mut tree = instantiate(markup, css);
        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();
        engine
            .compute_layout(&mut tree, viewport(400.0, 400.0))
            .unwrap();

        let root_box = tree.nodes.get(&tree.root).unwrap().layout;
        assert!((root_box.width - 200.0).abs() < 0.01);
        assert!((root_box.height - 100.0).abs() < 0.01);
        assert!((root_box.content_x - 40.0).abs() < 0.01);
        assert!((root_box.content_y - 10.0).abs() < 0.01);
        assert!((root_box.content_width - (200.0 - 40.0 - 20.0)).abs() < 0.01);
        assert!((root_box.content_height - (100.0 - 10.0 - 30.0)).abs() < 0.01);
    }

    #[test]
    fn changing_child_width_shifts_siblings_and_marks_paint() {
        let markup = r#"
            <ui id="root">
                <ui id="a"/>
                <ui id="b"/>
                <ui id="c"/>
            </ui>
        "#;
        let css = r#"
            #root { display: flex; flex-direction: row; width: 300px; height: 100px; }
            #a { width: 50px; height: 100px; }
            #b { width: 50px; height: 100px; }
            #c { width: 50px; height: 100px; }
        "#;
        let template = UiTemplate::from_src(markup, css).unwrap();
        let mut tree = DefaultUiTemplateInstantiator.instantiate(&template);
        tree.apply_style(&template.stylesheet).unwrap();

        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();
        engine
            .compute_layout(&mut tree, viewport(400.0, 400.0))
            .unwrap();

        for n in tree.nodes.values_mut() {
            n.dirty.remove(&DirtyFlag::Paint);
        }

        let root_children = tree.nodes.get(&tree.root).unwrap().children.clone();
        let (a_id, b_id, c_id) = (root_children[0], root_children[1], root_children[2]);
        assert!((tree.nodes.get(&a_id).unwrap().layout.x - 0.0).abs() < 0.01);
        assert!((tree.nodes.get(&b_id).unwrap().layout.x - 50.0).abs() < 0.01);
        assert!((tree.nodes.get(&c_id).unwrap().layout.x - 100.0).abs() < 0.01);

        {
            let a = tree.nodes.get_mut(&a_id).unwrap();
            a.computed_style.width = Length::Px(120.0);
            a.dirty.insert(DirtyFlag::Layout);
        }
        engine
            .compute_layout(&mut tree, viewport(400.0, 400.0))
            .unwrap();

        let b_x_after = tree.nodes.get(&b_id).unwrap().layout.x;
        let c_x_after = tree.nodes.get(&c_id).unwrap().layout.x;
        assert!((b_x_after - 120.0).abs() < 0.01, "b.x = {}", b_x_after);
        assert!((c_x_after - 170.0).abs() < 0.01, "c.x = {}", c_x_after);
        assert!(
            tree.nodes
                .get(&b_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Paint)
        );
        assert!(
            tree.nodes
                .get(&c_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Paint)
        );
    }

    #[test]
    fn sync_structure_drains_pending_removals() {
        let markup = r#"<ui id="root"><ui id="a"/></ui>"#;
        let mut tree = instantiate(markup, "");
        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();

        let child_ui_id = tree.nodes.get(&tree.root).unwrap().children[0];
        let child_taffy = engine.taffy_node_for(child_ui_id).unwrap();
        assert!(tree.pending_taffy_removals.is_empty());

        tree.remove_child(tree.root, child_ui_id);
        assert!(tree.pending_taffy_removals.contains(&child_ui_id));

        engine.sync_structure(&mut tree).unwrap();
        assert!(tree.pending_taffy_removals.is_empty());
        assert!(engine.taffy_node_for(child_ui_id).is_none());
        assert!(engine.ui_node_for(child_taffy).is_none());
    }

    #[test]
    fn text_node_gets_measured_by_fallback_when_no_font() {
        // A <text> leaf with no registered font falls back to an estimate
        // (font_size * 0.6 * char_count). Verify the text child ends up with a
        // non-zero layout box inside a fixed-width parent.
        let markup = r#"<ui id="root"><text>Hello</text></ui>"#;
        let css = r#"#root { width: 200px; height: 50px; }"#;
        let mut tree = instantiate(markup, css);
        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();
        engine
            .compute_layout(&mut tree, viewport(400.0, 400.0))
            .unwrap();

        // The root should be 200×50.
        let root_box = tree.nodes.get(&tree.root).unwrap().layout;
        assert!((root_box.width - 200.0).abs() < 0.5);

        // The text child should have a positive width (fallback estimate).
        let text_child = tree.nodes.get(&tree.root).unwrap().children[0];
        let text_box = tree.nodes.get(&text_child).unwrap().layout;
        assert!(
            text_box.width > 0.0,
            "text node should have non-zero width, got {}",
            text_box.width
        );
    }
}
