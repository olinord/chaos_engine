use std::collections::{HashMap, HashSet};

use crate::ui::{
    NodeId,
    ast::{UiMarkupNode, UiStyleSheetAst},
    layout::LayoutBox,
    style::{ComputedStyle, Declaration, PseudoState, StyleChange, StyleRule, StyleValue},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DirtyFlag {
    Structure,
    Style,
    Layout,
    Paint,
}

#[derive(Clone, Debug, Default, Eq, Hash)]
pub enum UiTag {
    Text(String),
    Button,
    UI,
    #[default]
    Invalid,
}

#[derive(Clone, Debug, Default)]
pub struct UiNodeState {
    pub pseudo_states: HashSet<PseudoState>,
}

#[derive(Clone, Debug)]
pub struct UiNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub tag: UiTag,
    pub id_attr: Option<String>,
    pub class_list: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub inline_style: Option<Vec<Declaration>>,
    pub state: UiNodeState,
    pub computed_style: ComputedStyle,
    pub layout: LayoutBox,
    pub dirty: HashSet<DirtyFlag>,
}

#[derive(Default)]
pub struct UiTree {
    pub root: NodeId,
    pub nodes: HashMap<NodeId, UiNode>,
    pub next_id: NodeId,
    pub focused_node: Option<NodeId>,
    pub hovered_node: Option<NodeId>,
    pub active_node: Option<NodeId>,
    /// Node ids removed from the tree since the last structure sync. Phase B
    /// drains this to delete the corresponding taffy nodes.
    pub pending_taffy_removals: Vec<NodeId>,
}

impl UiTree {
    pub fn new() -> Self {
        Self {
            root: 0,
            nodes: HashMap::new(),
            next_id: 1,
            focused_node: None,
            hovered_node: None,
            active_node: None,
            pending_taffy_removals: Vec::new(),
        }
    }

    pub fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn mark_dirty(&mut self, node: NodeId, flag: DirtyFlag) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.dirty.insert(flag);
        }
    }

    pub fn mark_subtree_dirty(&mut self, node: NodeId, flag: DirtyFlag) {
        let mut children_nodes = vec![];
        if let Some(n) = self.nodes.get_mut(&node) {
            n.dirty.insert(flag);
            children_nodes = n.children.clone();
        }

        for &child_id in &children_nodes {
            self.mark_subtree_dirty(child_id, flag);
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&UiNode> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut UiNode> {
        self.nodes.get_mut(&id)
    }

    /// Attach `node` under `parent`. The caller owns id allocation via
    /// `alloc_id`. Marks `Structure` on the parent, `Layout` on existing
    /// siblings, and `Style`+`Layout` on the new subtree.
    pub fn add_child(&mut self, parent: NodeId, mut node: UiNode) -> Option<NodeId> {
        if !self.nodes.contains_key(&parent) {
            return None;
        }
        let id = node.id;
        node.parent = Some(parent);
        node.dirty.insert(DirtyFlag::Style);
        node.dirty.insert(DirtyFlag::Layout);

        let existing_siblings: Vec<NodeId> = self
            .nodes
            .get(&parent)
            .map(|p| p.children.clone())
            .unwrap_or_default();
        for sibling in existing_siblings {
            if let Some(s) = self.nodes.get_mut(&sibling) {
                s.dirty.insert(DirtyFlag::Layout);
            }
        }

        // Descendants attached inside `node.children` are already registered
        // in the tree elsewhere; only the freshly-inserted root of the new
        // subtree needs its initial-dirty treatment.
        self.nodes.insert(id, node);

        let parent_node = self.nodes.get_mut(&parent).expect("parent checked above");
        parent_node.children.push(id);
        parent_node.dirty.insert(DirtyFlag::Structure);
        Some(id)
    }

    /// Detach `child` from `parent` and remove it (and its descendants) from
    /// the tree. Removed node ids are appended to `pending_taffy_removals` so
    /// Phase B can delete the corresponding taffy nodes.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        let Some(parent_node) = self.nodes.get_mut(&parent) else {
            return false;
        };
        let Some(pos) = parent_node.children.iter().position(|&c| c == child) else {
            return false;
        };
        parent_node.children.remove(pos);
        parent_node.dirty.insert(DirtyFlag::Structure);

        let remaining_siblings = parent_node.children.clone();
        for sib in remaining_siblings {
            if let Some(s) = self.nodes.get_mut(&sib) {
                s.dirty.insert(DirtyFlag::Layout);
            }
        }

        let mut to_remove = vec![child];
        let mut stack = vec![child];
        while let Some(id) = stack.pop() {
            if let Some(n) = self.nodes.get(&id) {
                for &c in &n.children {
                    to_remove.push(c);
                    stack.push(c);
                }
            }
        }
        for id in to_remove {
            self.nodes.remove(&id);
            self.pending_taffy_removals.push(id);
        }
        true
    }

    pub fn add_class(&mut self, node: NodeId, class: &str) {
        if let Some(n) = self.nodes.get_mut(&node) {
            if !n.class_list.iter().any(|c| c == class) {
                n.class_list.push(class.to_string());
                n.dirty.insert(DirtyFlag::Style);
            }
        }
    }

    pub fn remove_class(&mut self, node: NodeId, class: &str) {
        if let Some(n) = self.nodes.get_mut(&node) {
            if let Some(pos) = n.class_list.iter().position(|c| c == class) {
                n.class_list.remove(pos);
                n.dirty.insert(DirtyFlag::Style);
            }
        }
    }

    pub fn set_id_attr(&mut self, node: NodeId, id_attr: Option<String>) {
        if let Some(n) = self.nodes.get_mut(&node) {
            if n.id_attr != id_attr {
                n.id_attr = id_attr;
                n.dirty.insert(DirtyFlag::Style);
            }
        }
    }

    pub fn set_attribute(&mut self, node: NodeId, key: &str, value: Option<String>) {
        if let Some(n) = self.nodes.get_mut(&node) {
            let changed = match &value {
                Some(v) => n.attributes.insert(key.to_string(), v.clone()).as_ref() != Some(v),
                None => n.attributes.remove(key).is_some(),
            };
            if changed {
                n.dirty.insert(DirtyFlag::Style);
            }
        }
    }

    pub fn set_pseudo_state(&mut self, node: NodeId, state: PseudoState, active: bool) {
        if let Some(n) = self.nodes.get_mut(&node) {
            let changed = if active {
                n.state.pseudo_states.insert(state)
            } else {
                n.state.pseudo_states.remove(&state)
            };
            if changed {
                n.dirty.insert(DirtyFlag::Style);
            }
        }
    }

    /// Mark every descendant of `node` as `Style`-dirty. Used by the style
    /// pass when an inherited property changes.
    pub fn mark_descendants_style_dirty(&mut self, node: NodeId) {
        let mut stack: Vec<NodeId> = self
            .nodes
            .get(&node)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        while let Some(id) = stack.pop() {
            if let Some(n) = self.nodes.get_mut(&id) {
                n.dirty.insert(DirtyFlag::Style);
                stack.extend(n.children.clone());
            }
        }
    }

    /// Recompute styles for every `Style`-dirty node until a fixed point is
    /// reached. Inherited changes propagate to descendants across iterations.
    pub fn apply_style(&mut self, sheet: &UiStyleSheetAst) -> Result<(), String> {
        loop {
            let dirty_ids: Vec<NodeId> = self
                .nodes
                .iter()
                .filter(|(_, n)| n.dirty.contains(&DirtyFlag::Style))
                .map(|(id, _)| *id)
                .collect();
            if dirty_ids.is_empty() {
                break;
            }

            for id in dirty_ids {
                let parent_style = self
                    .nodes
                    .get(&id)
                    .and_then(|node| node.parent)
                    .and_then(|parent_id| self.nodes.get(&parent_id))
                    .map(|parent| parent.computed_style.clone());

                let change = {
                    let Some(node) = self.nodes.get_mut(&id) else {
                        continue;
                    };
                    node.dirty.remove(&DirtyFlag::Style);
                    node.compute_style(&sheet.rules, parent_style.as_ref())
                };
                match change {
                    StyleChange::NoChange => {}
                    StyleChange::PaintOnly => {
                        if let Some(n) = self.nodes.get_mut(&id) {
                            n.dirty.insert(DirtyFlag::Paint);
                        }
                    }
                    StyleChange::LayoutAndPaint => {
                        if let Some(n) = self.nodes.get_mut(&id) {
                            n.dirty.insert(DirtyFlag::Layout);
                        }
                    }
                    StyleChange::LayoutAndPaintAndInherited => {
                        if let Some(n) = self.nodes.get_mut(&id) {
                            n.dirty.insert(DirtyFlag::Layout);
                        }
                        self.mark_descendants_style_dirty(id);
                    }
                }
            }
        }
        Ok(())
    }
}

fn convert_markup_to_tree(
    markup_node: UiMarkupNode,
    tree: &mut UiTree,
    parent: Option<NodeId>,
) -> NodeId {
    let node_id = tree.alloc_id();
    let ui_node = UiNode {
        id: node_id,
        parent,
        children: Vec::new(),
        tag: markup_node.tag,
        id_attr: markup_node.id,
        class_list: markup_node.classes,
        attributes: markup_node.attributes,
        inline_style: markup_node.inline_style,
        state: UiNodeState::default(),
        computed_style: ComputedStyle::default(),
        layout: LayoutBox::default(),
        dirty: HashSet::from_iter([DirtyFlag::Style, DirtyFlag::Layout]),
    };

    tree.nodes.insert(node_id, ui_node);

    for child_markup in markup_node.children {
        let child_id = convert_markup_to_tree(child_markup, tree, Some(node_id));
        if let Some(parent_node) = tree.nodes.get_mut(&node_id) {
            parent_node.children.push(child_id);
        }
    }

    node_id
}

pub struct UiTemplate {
    pub root_markup: UiMarkupNode,
    pub stylesheet: UiStyleSheetAst,
}

impl UiTemplate {
    pub fn from_src(markup_src: &str, stylesheet_src: &str) -> Result<Self, String> {
        let root_markup = UiMarkupNode::from_src(markup_src)?;
        let stylesheet = UiStyleSheetAst::from_stylesheet(stylesheet_src)?;
        Ok(Self {
            root_markup,
            stylesheet,
        })
    }
}

pub trait UiTemplateInstantiator {
    fn instantiate(&self, template: &UiTemplate) -> UiTree;
}

pub struct DefaultUiTemplateInstantiator;

impl UiTemplateInstantiator for DefaultUiTemplateInstantiator {
    fn instantiate(&self, template: &UiTemplate) -> UiTree {
        let mut uitree = UiTree::new();
        let root_id = convert_markup_to_tree(template.root_markup.clone(), &mut uitree, None);
        uitree.root = root_id;
        uitree
    }
}

impl TryFrom<String> for UiTag {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "text" => Ok(UiTag::Text("".to_string())), // Placeholder for text content
            "ui" => Ok(UiTag::UI),
            "button" => Ok(UiTag::Button),
            _ => Err(format!("Invalid UI tag: {}", value)),
        }
    }
}

impl PartialEq for UiTag {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (UiTag::Text(_), UiTag::Text(_)) => true,
            (UiTag::UI, UiTag::UI) => true,
            (UiTag::Button, UiTag::Button) => true,
            _ => false,
        }
    }
}

impl UiNode {
    /// Recompute the node's `ComputedStyle` from `rules` (plus any inline
    /// style declarations stored on the node). Diffs against the previous
    /// value and returns the classification the style pass uses to decide
    /// which downstream flags to raise.
    pub fn compute_style(
        &mut self,
        rules: &[StyleRule],
        parent_style: Option<&ComputedStyle>,
    ) -> StyleChange {
        let mut applied_rules: Vec<&StyleRule> = rules
            .iter()
            .filter(|rule| rule.selector.matches_ui_node(self))
            .collect();

        applied_rules.sort_by(|a, b| {
            a.specificity
                .cmp(&b.specificity)
                .then(a.source_order.cmp(&b.source_order))
        });

        let mut scratch = ComputedStyle::default();
        if let Some(parent) = parent_style {
            inherit_parent_style(&mut scratch, parent);
        }
        for rule in applied_rules {
            for declaration in &rule.declarations {
                apply_declaration(&mut scratch, declaration.value.clone());
            }
        }
        // Inline style has specificity 1000 — apply after all matched rules
        // so it wins any tie against the highest-specificity selector rule.
        if let Some(inline) = &self.inline_style {
            for declaration in inline {
                apply_declaration(&mut scratch, declaration.value.clone());
            }
        }

        let change = diff_computed_style(&self.computed_style, &scratch);
        self.computed_style = scratch;
        change
    }
}

fn inherit_parent_style(style: &mut ComputedStyle, parent: &ComputedStyle) {
    style.text_color = parent.text_color;
    style.font_family = parent.font_family.clone();
    style.font_size = parent.font_size;
    style.text_align = parent.text_align;
    style.visibility = parent.visibility;
}

fn apply_declaration(style: &mut ComputedStyle, value: StyleValue) {
    match value {
        StyleValue::Display(v) => style.display = v,
        StyleValue::Width(v) => style.width = v,
        StyleValue::Height(v) => style.height = v,
        StyleValue::MinWidth(v) => style.min_width = v,
        StyleValue::MinHeight(v) => style.min_height = v,
        StyleValue::MaxWidth(v) => style.max_width = v,
        StyleValue::MaxHeight(v) => style.max_height = v,
        StyleValue::Margin(v) => style.margin = v,
        StyleValue::Padding(v) => style.padding = v,
        StyleValue::Gap(v) => style.gap = v,
        StyleValue::FlexDirection(v) => style.flex_direction = v,
        StyleValue::FlexGrow(v) => style.flex_grow = v,
        StyleValue::FlexShrink(v) => style.flex_shrink = v,
        StyleValue::AlignItems(v) => style.align_items = v,
        StyleValue::JustifyContent(v) => style.justify_content = v,
        StyleValue::BackgroundColor(v) => style.background_color = Some(v),
        StyleValue::BorderColor(v) => style.border_color = Some(v),
        StyleValue::BorderWidth(v) => style.border_width = v,
        StyleValue::BorderRadius(v) => style.border_radius = v,
        StyleValue::Opacity(v) => style.opacity = v,
        StyleValue::FontFamily(v) => style.font_family = v,
        StyleValue::FontSize(v) => style.font_size = v,
        StyleValue::TextColor(v) => style.text_color = v,
        StyleValue::TextAlign(v) => style.text_align = v,
        StyleValue::ZIndex(v) => style.z_index = v,
        StyleValue::PointerEvents(v) => style.pointer_events = v,
        StyleValue::Visibility(v) => style.visibility = v,
        StyleValue::Overflow(v) => style.overflow = v,
    }
}

fn diff_computed_style(old: &ComputedStyle, new: &ComputedStyle) -> StyleChange {
    let layout_changed = old.display != new.display
        || old.width != new.width
        || old.height != new.height
        || old.min_width != new.min_width
        || old.min_height != new.min_height
        || old.max_width != new.max_width
        || old.max_height != new.max_height
        || old.margin != new.margin
        || old.padding != new.padding
        || old.gap != new.gap
        || old.border_width != new.border_width
        || old.flex_direction != new.flex_direction
        || old.flex_grow != new.flex_grow
        || old.flex_shrink != new.flex_shrink
        || old.align_items != new.align_items
        || old.justify_content != new.justify_content
        || old.overflow != new.overflow;

    let inherited_changed = old.text_color != new.text_color
        || old.font_family != new.font_family
        || old.font_size != new.font_size
        || old.text_align != new.text_align
        || old.visibility != new.visibility;

    let paint_changed = old.background_color != new.background_color
        || old.border_color != new.border_color
        || old.border_radius != new.border_radius
        || old.opacity != new.opacity
        || old.z_index != new.z_index
        || old.pointer_events != new.pointer_events;

    if inherited_changed {
        StyleChange::LayoutAndPaintAndInherited
    } else if layout_changed {
        StyleChange::LayoutAndPaint
    } else if paint_changed {
        StyleChange::PaintOnly
    } else {
        StyleChange::NoChange
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::ast::UiStyleSheetAst;
    use crate::ui::colors::Color;
    use crate::ui::style::Length;

    fn make_node(id: NodeId, tag: UiTag) -> UiNode {
        UiNode {
            id,
            parent: None,
            children: vec![],
            tag,
            id_attr: None,
            class_list: vec![],
            attributes: HashMap::new(),
            inline_style: None,
            state: UiNodeState::default(),
            computed_style: ComputedStyle::default(),
            layout: LayoutBox::default(),
            dirty: HashSet::new(),
        }
    }

    fn make_tree_with_parent_child() -> (UiTree, NodeId, NodeId) {
        let mut tree = UiTree::new();
        let parent_id = tree.alloc_id();
        let child_id = tree.alloc_id();

        let mut parent = make_node(parent_id, UiTag::UI);
        parent.children = vec![child_id];
        tree.nodes.insert(parent_id, parent);

        let mut child = make_node(child_id, UiTag::Text("text".into()));
        child.parent = Some(parent_id);
        tree.nodes.insert(child_id, child);

        (tree, parent_id, child_id)
    }

    #[test]
    fn alloc_id_increments() {
        let mut tree = UiTree::new();
        assert_eq!(tree.alloc_id(), 1);
        assert_eq!(tree.alloc_id(), 2);
        assert_eq!(tree.alloc_id(), 3);
    }

    #[test]
    fn node_lookup_returns_correct_node() {
        let (tree, parent_id, child_id) = make_tree_with_parent_child();
        assert!(tree.node(parent_id).is_some());
        assert!(tree.node(child_id).is_some());
        assert!(tree.node(99).is_none());
    }

    #[test]
    fn node_mut_allows_mutation() {
        let (mut tree, parent_id, _) = make_tree_with_parent_child();
        tree.node_mut(parent_id).unwrap().id_attr = Some("root".into());
        assert_eq!(
            tree.node(parent_id).unwrap().id_attr.as_deref(),
            Some("root")
        );
    }

    #[test]
    fn mark_dirty_sets_flag_on_node() {
        let (mut tree, parent_id, child_id) = make_tree_with_parent_child();
        tree.mark_dirty(parent_id, DirtyFlag::Style);
        assert!(
            tree.node(parent_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Style)
        );
        assert!(
            !tree
                .node(child_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Style)
        );
    }

    #[test]
    fn mark_dirty_on_missing_node_is_noop() {
        let (mut tree, _, _) = make_tree_with_parent_child();
        tree.mark_dirty(99, DirtyFlag::Layout); // must not panic
    }

    #[test]
    fn mark_subtree_dirty_propagates_to_children() {
        let (mut tree, parent_id, child_id) = make_tree_with_parent_child();
        tree.mark_subtree_dirty(parent_id, DirtyFlag::Layout);
        assert!(
            tree.node(parent_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Layout)
        );
        assert!(
            tree.node(child_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Layout)
        );
    }

    #[test]
    fn mark_subtree_dirty_does_not_affect_siblings() {
        let mut tree = UiTree::new();
        let root_id = tree.alloc_id();
        let child_a = tree.alloc_id();
        let child_b = tree.alloc_id();

        for (id, parent, children) in [
            (root_id, None, vec![child_a, child_b]),
            (child_a, Some(root_id), vec![]),
            (child_b, Some(root_id), vec![]),
        ] {
            let mut node = make_node(id, UiTag::UI);
            node.parent = parent;
            node.children = children;
            tree.nodes.insert(id, node);
        }

        tree.mark_subtree_dirty(child_a, DirtyFlag::Paint);
        assert!(
            tree.node(child_a)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Paint)
        );
        assert!(
            !tree
                .node(child_b)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Paint)
        );
        assert!(
            !tree
                .node(root_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Paint)
        );
    }

    #[test]
    fn instantiate_builds_tree_from_template() {
        let template =
            UiTemplate::from_src(r#"<ui id="root"><text>Hello</text></ui>"#, "").unwrap();

        let tree = DefaultUiTemplateInstantiator.instantiate(&template);

        assert_eq!(tree.nodes.len(), 2);
        let root = tree.node(tree.root).unwrap();
        assert_eq!(root.id_attr.as_deref(), Some("root"));
        assert_eq!(root.children.len(), 1);

        let child = tree.node(root.children[0]).unwrap();
        assert!(matches!(&child.tag, UiTag::Text(content) if content == "Hello"));
        assert_eq!(child.parent, Some(tree.root));
    }

    #[test]
    fn instantiate_wires_parent_refs() {
        let template = UiTemplate::from_src(r#"<ui><ui><text>deep</text></ui></ui>"#, "").unwrap();

        let tree = DefaultUiTemplateInstantiator.instantiate(&template);

        let root = tree.node(tree.root).unwrap();
        let mid_id = root.children[0];
        let mid = tree.node(mid_id).unwrap();
        let leaf = tree.node(mid.children[0]).unwrap();

        assert_eq!(mid.parent, Some(tree.root));
        assert_eq!(leaf.parent, Some(mid_id));
    }

    // ---------- Phase A: mutation API, diff-driven apply_style ----------

    fn parent_child_tree_with_class(
        parent_class: &str,
        child_class: &str,
    ) -> (UiTree, NodeId, NodeId) {
        let (mut tree, parent_id, child_id) = make_tree_with_parent_child();
        tree.root = parent_id;
        tree.node_mut(parent_id)
            .unwrap()
            .class_list
            .push(parent_class.to_string());
        tree.node_mut(child_id)
            .unwrap()
            .class_list
            .push(child_class.to_string());
        // Start clean.
        tree.node_mut(parent_id).unwrap().dirty.clear();
        tree.node_mut(child_id).unwrap().dirty.clear();
        (tree, parent_id, child_id)
    }

    #[test]
    fn compute_style_returns_no_change_when_rules_dont_match() {
        let mut node = make_node(1, UiTag::UI);
        let sheet = UiStyleSheetAst::from_stylesheet("").unwrap();
        assert_eq!(
            node.compute_style(&sheet.rules, None),
            StyleChange::NoChange
        );
    }

    #[test]
    fn compute_style_paint_only_for_background_color_change() {
        let mut node = make_node(1, UiTag::UI);
        node.class_list.push("panel".into());
        let sheet = UiStyleSheetAst::from_stylesheet(".panel { background-color: red; }").unwrap();
        assert_eq!(
            node.compute_style(&sheet.rules, None),
            StyleChange::PaintOnly
        );
        assert_eq!(
            node.computed_style.background_color,
            Some(Color::rgb(255, 0, 0))
        );
    }

    #[test]
    fn compute_style_layout_and_paint_for_width_change() {
        let mut node = make_node(1, UiTag::UI);
        node.class_list.push("wide".into());
        let sheet = UiStyleSheetAst::from_stylesheet(".wide { width: 100px; }").unwrap();
        assert_eq!(
            node.compute_style(&sheet.rules, None),
            StyleChange::LayoutAndPaint
        );
        assert_eq!(node.computed_style.width, Length::Px(100.0));
    }

    #[test]
    fn compute_style_inherited_for_text_color_change() {
        let mut node = make_node(1, UiTag::UI);
        node.class_list.push("dark".into());
        let sheet = UiStyleSheetAst::from_stylesheet(".dark { color: #000000; }").unwrap();
        assert_eq!(
            node.compute_style(&sheet.rules, None),
            StyleChange::LayoutAndPaintAndInherited
        );
    }

    #[test]
    fn compute_style_applies_inline_style_over_matched_rules() {
        let mut node = make_node(1, UiTag::UI);
        node.class_list.push("panel".into());
        node.inline_style = Some(crate::ui::style::Declaration::from_src("width: 42px;").unwrap());
        let sheet = UiStyleSheetAst::from_stylesheet(".panel { width: 10px; }").unwrap();
        node.compute_style(&sheet.rules, None);
        assert_eq!(node.computed_style.width, Length::Px(42.0));
    }

    #[test]
    fn child_text_inherits_parent_text_color() {
        let mut parent = make_node(1, UiTag::UI);
        parent.class_list.push("green_panel".into());

        let child = make_node(2, UiTag::Text("Panel B".into()));

        let sheet = UiStyleSheetAst::from_stylesheet(".green_panel { color: #00ff00; }").unwrap();

        assert_eq!(
            parent.compute_style(&sheet.rules, None),
            StyleChange::LayoutAndPaintAndInherited
        );

        let mut child = child;
        child.compute_style(&sheet.rules, Some(&parent.computed_style));

        assert_eq!(child.computed_style.text_color, Color::rgb(0, 255, 0));
    }

    #[test]
    fn hover_toggle_marks_paint_only_on_node() {
        let (mut tree, parent_id, child_id) = parent_child_tree_with_class("panel", "");
        let sheet = UiStyleSheetAst::from_stylesheet(
            ".panel { background-color: red; } .panel:hover { background-color: blue; }",
        )
        .unwrap();

        // Prime computed style so subsequent toggles diff cleanly.
        tree.node_mut(parent_id)
            .unwrap()
            .dirty
            .insert(DirtyFlag::Style);
        tree.node_mut(child_id)
            .unwrap()
            .dirty
            .insert(DirtyFlag::Style);
        tree.apply_style(&sheet).unwrap();
        for id in [parent_id, child_id] {
            tree.node_mut(id).unwrap().dirty.clear();
        }

        tree.set_pseudo_state(parent_id, PseudoState::Hover, true);
        tree.apply_style(&sheet).unwrap();

        let parent = tree.node(parent_id).unwrap();
        assert!(parent.dirty.contains(&DirtyFlag::Paint));
        assert!(!parent.dirty.contains(&DirtyFlag::Layout));
        let child = tree.node(child_id).unwrap();
        assert!(!child.dirty.contains(&DirtyFlag::Style));
        assert!(!child.dirty.contains(&DirtyFlag::Layout));
        assert!(!child.dirty.contains(&DirtyFlag::Paint));
    }

    #[test]
    fn width_change_marks_layout_not_descendants() {
        let (mut tree, parent_id, child_id) = parent_child_tree_with_class("panel", "leaf");
        let sheet = UiStyleSheetAst::from_stylesheet(".big { width: 100px; }").unwrap();

        tree.add_class(parent_id, "big");
        tree.apply_style(&sheet).unwrap();

        let parent = tree.node(parent_id).unwrap();
        assert!(parent.dirty.contains(&DirtyFlag::Layout));
        assert!(!parent.dirty.contains(&DirtyFlag::Style));
        let child = tree.node(child_id).unwrap();
        assert!(!child.dirty.contains(&DirtyFlag::Style));
        assert!(!child.dirty.contains(&DirtyFlag::Layout));
    }

    #[test]
    fn inherited_class_change_propagates_style_to_descendants() {
        let (mut tree, parent_id, child_id) = parent_child_tree_with_class("panel", "leaf");
        // Prime baseline.
        let baseline = UiStyleSheetAst::from_stylesheet("").unwrap();
        tree.node_mut(parent_id)
            .unwrap()
            .dirty
            .insert(DirtyFlag::Style);
        tree.node_mut(child_id)
            .unwrap()
            .dirty
            .insert(DirtyFlag::Style);
        tree.apply_style(&baseline).unwrap();
        for id in [parent_id, child_id] {
            tree.node_mut(id).unwrap().dirty.clear();
        }

        // Rule that changes an inherited property (color) on the parent
        // and a rule that would mark the child if it gets re-evaluated.
        let sheet = UiStyleSheetAst::from_stylesheet(
            ".dark { color: #000000; } .leaf { background-color: red; }",
        )
        .unwrap();
        // First run with new rules to reach a stable baseline where .leaf
        // has picked up its rule (child recomputes because it starts dirty
        // after we call add_class below only for the parent).
        //
        // Instead, prime just the child with the new sheet so the leaf's
        // background rule has already been consumed before we mutate.
        tree.node_mut(child_id)
            .unwrap()
            .dirty
            .insert(DirtyFlag::Style);
        tree.apply_style(&sheet).unwrap();
        for id in [parent_id, child_id] {
            tree.node_mut(id).unwrap().dirty.clear();
        }

        // Now mutate parent to trigger an inherited-property change.
        tree.add_class(parent_id, "dark");
        tree.apply_style(&sheet).unwrap();

        // Parent's inherited change forced its Layout flag …
        let parent = tree.node(parent_id).unwrap();
        assert!(parent.dirty.contains(&DirtyFlag::Layout));

        // … and the pass re-ran the child (its Style dirty was consumed).
        // Since the child's own rules didn't change, it produces NoChange
        // and therefore has no Paint/Layout flag from this pass. Verify via
        // the visited-side-effect: computed_style has been (re)assigned to
        // reflect the .leaf rule.
        let child = tree.node(child_id).unwrap();
        assert!(!child.dirty.contains(&DirtyFlag::Style));
        assert_eq!(
            child.computed_style.background_color,
            Some(Color::rgb(255, 0, 0))
        );
    }

    #[test]
    fn mark_descendants_style_dirty_walks_subtree() {
        let mut tree = UiTree::new();
        let root = tree.alloc_id();
        let mid = tree.alloc_id();
        let leaf = tree.alloc_id();
        let sibling = tree.alloc_id();
        let mut root_node = make_node(root, UiTag::UI);
        root_node.children = vec![mid, sibling];
        tree.nodes.insert(root, root_node);
        let mut mid_node = make_node(mid, UiTag::UI);
        mid_node.parent = Some(root);
        mid_node.children = vec![leaf];
        tree.nodes.insert(mid, mid_node);
        let mut leaf_node = make_node(leaf, UiTag::Text("t".into()));
        leaf_node.parent = Some(mid);
        tree.nodes.insert(leaf, leaf_node);
        let mut sib = make_node(sibling, UiTag::UI);
        sib.parent = Some(root);
        tree.nodes.insert(sibling, sib);

        tree.mark_descendants_style_dirty(mid);

        assert!(!tree.node(root).unwrap().dirty.contains(&DirtyFlag::Style));
        assert!(!tree.node(mid).unwrap().dirty.contains(&DirtyFlag::Style));
        assert!(tree.node(leaf).unwrap().dirty.contains(&DirtyFlag::Style));
        assert!(
            !tree
                .node(sibling)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Style)
        );
    }

    #[test]
    fn add_child_marks_structure_on_parent_and_layout_on_siblings() {
        let mut tree = UiTree::new();
        let root_id = tree.alloc_id();
        let existing_id = tree.alloc_id();
        let mut root_node = make_node(root_id, UiTag::UI);
        root_node.children = vec![existing_id];
        tree.nodes.insert(root_id, root_node);
        let mut existing = make_node(existing_id, UiTag::UI);
        existing.parent = Some(root_id);
        tree.nodes.insert(existing_id, existing);
        tree.root = root_id;
        for id in [root_id, existing_id] {
            tree.node_mut(id).unwrap().dirty.clear();
        }

        let new_id = tree.alloc_id();
        let new_node = make_node(new_id, UiTag::UI);
        assert_eq!(tree.add_child(root_id, new_node), Some(new_id));

        let parent = tree.node(root_id).unwrap();
        assert!(parent.dirty.contains(&DirtyFlag::Structure));
        assert!(parent.children.contains(&new_id));

        let existing = tree.node(existing_id).unwrap();
        assert!(existing.dirty.contains(&DirtyFlag::Layout));

        let added = tree.node(new_id).unwrap();
        assert!(added.dirty.contains(&DirtyFlag::Style));
        assert!(added.dirty.contains(&DirtyFlag::Layout));
        assert_eq!(added.parent, Some(root_id));
    }

    #[test]
    fn add_child_returns_none_for_unknown_parent() {
        let mut tree = UiTree::new();
        let orphan = make_node(1, UiTag::UI);
        assert!(tree.add_child(99, orphan).is_none());
    }

    #[test]
    fn remove_child_populates_pending_taffy_removals_with_subtree() {
        let mut tree = UiTree::new();
        let root_id = tree.alloc_id();
        let mid_id = tree.alloc_id();
        let leaf_id = tree.alloc_id();
        let mut root_node = make_node(root_id, UiTag::UI);
        root_node.children = vec![mid_id];
        tree.nodes.insert(root_id, root_node);
        let mut mid = make_node(mid_id, UiTag::UI);
        mid.parent = Some(root_id);
        mid.children = vec![leaf_id];
        tree.nodes.insert(mid_id, mid);
        let mut leaf = make_node(leaf_id, UiTag::Text("t".into()));
        leaf.parent = Some(mid_id);
        tree.nodes.insert(leaf_id, leaf);
        tree.root = root_id;
        for id in [root_id, mid_id, leaf_id] {
            tree.node_mut(id).unwrap().dirty.clear();
        }

        assert!(tree.remove_child(root_id, mid_id));

        assert!(tree.node(mid_id).is_none());
        assert!(tree.node(leaf_id).is_none());
        assert!(tree.node(root_id).unwrap().children.is_empty());
        assert!(
            tree.node(root_id)
                .unwrap()
                .dirty
                .contains(&DirtyFlag::Structure)
        );
        assert!(tree.pending_taffy_removals.contains(&mid_id));
        assert!(tree.pending_taffy_removals.contains(&leaf_id));
    }

    #[test]
    fn remove_child_marks_layout_on_remaining_siblings() {
        let mut tree = UiTree::new();
        let root_id = tree.alloc_id();
        let a_id = tree.alloc_id();
        let b_id = tree.alloc_id();
        let mut root_node = make_node(root_id, UiTag::UI);
        root_node.children = vec![a_id, b_id];
        tree.nodes.insert(root_id, root_node);
        let mut a = make_node(a_id, UiTag::UI);
        a.parent = Some(root_id);
        tree.nodes.insert(a_id, a);
        let mut b = make_node(b_id, UiTag::UI);
        b.parent = Some(root_id);
        tree.nodes.insert(b_id, b);
        for id in [root_id, a_id, b_id] {
            tree.node_mut(id).unwrap().dirty.clear();
        }

        tree.remove_child(root_id, a_id);
        assert!(tree.node(b_id).unwrap().dirty.contains(&DirtyFlag::Layout));
    }

    #[test]
    fn add_class_marks_style_dirty_only_when_new() {
        let mut tree = UiTree::new();
        let id = tree.alloc_id();
        tree.nodes.insert(id, make_node(id, UiTag::UI));

        tree.add_class(id, "foo");
        assert!(tree.node(id).unwrap().dirty.contains(&DirtyFlag::Style));
        assert_eq!(tree.node(id).unwrap().class_list, vec!["foo".to_string()]);

        tree.node_mut(id).unwrap().dirty.clear();
        tree.add_class(id, "foo");
        assert!(!tree.node(id).unwrap().dirty.contains(&DirtyFlag::Style));
    }

    #[test]
    fn set_pseudo_state_marks_style_dirty_on_change() {
        let mut tree = UiTree::new();
        let id = tree.alloc_id();
        tree.nodes.insert(id, make_node(id, UiTag::UI));

        tree.set_pseudo_state(id, PseudoState::Hover, true);
        assert!(tree.node(id).unwrap().dirty.contains(&DirtyFlag::Style));
        tree.node_mut(id).unwrap().dirty.clear();

        tree.set_pseudo_state(id, PseudoState::Hover, true);
        assert!(!tree.node(id).unwrap().dirty.contains(&DirtyFlag::Style));

        tree.set_pseudo_state(id, PseudoState::Hover, false);
        assert!(tree.node(id).unwrap().dirty.contains(&DirtyFlag::Style));
    }

    #[test]
    fn instantiated_tree_has_initial_dirty_style_and_layout() {
        let template = UiTemplate::from_src(r#"<ui><text>hi</text></ui>"#, "").unwrap();
        let tree = DefaultUiTemplateInstantiator.instantiate(&template);
        for node in tree.nodes.values() {
            assert!(node.dirty.contains(&DirtyFlag::Style));
            assert!(node.dirty.contains(&DirtyFlag::Layout));
        }
    }

    #[test]
    fn inline_style_from_markup_is_preserved_on_node() {
        let template = UiTemplate::from_src(r#"<ui style="width: 42px;"></ui>"#, "").unwrap();
        let tree = DefaultUiTemplateInstantiator.instantiate(&template);
        let root = tree.node(tree.root).unwrap();
        let inline = root.inline_style.as_ref().expect("inline style copied");
        assert_eq!(inline.len(), 1);
    }
}
