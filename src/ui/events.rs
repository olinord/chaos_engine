use crate::{
    device::events::{ChaosKeyCode, ChaosMouseButton},
    ui::{
        NodeId,
        paint::{PaintCommand, PaintList},
        runtime::{DirtyFlag, UiTree},
        style::PseudoState,
    },
};

#[derive(Clone, Copy, Debug)]
pub struct PointerPos {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub enum UiInputEvent {
    PointerMove(PointerPos),
    PointerDown {
        pos: PointerPos,
        button: ChaosMouseButton,
    },
    PointerUp {
        pos: PointerPos,
        button: ChaosMouseButton,
    },
    Scroll {
        dx: f32,
        dy: f32,
    },
    KeyDown {
        key: ChaosKeyCode,
    },
    KeyUp {
        key: ChaosKeyCode,
    },
    TextInput {
        text: String,
    },
}

#[derive(Clone, Debug)]
pub enum UiEvent {
    PointerEnter { node: NodeId },
    PointerLeave { node: NodeId },
    Click { node: NodeId },
    FocusGained { node: NodeId },
    FocusLost { node: NodeId },
    ValueChanged { node: NodeId, value: String },
}

pub struct UiEventRouter;

impl UiEventRouter {
    pub fn dispatch_input(
        tree: &mut UiTree,
        paint_list: &PaintList,
        input: UiInputEvent,
        out_events: &mut Vec<UiEvent>,
    ) -> Result<(), String> {
        match input {
            UiInputEvent::PointerMove(pos) => {
                let hit = Self::hit_test(paint_list, tree, pos);
                let prev = tree.hovered_node;
                if prev != hit {
                    let old_chain = Self::ancestor_chain(tree, prev);
                    let new_chain = Self::ancestor_chain(tree, hit);
                    Self::update_pseudo_chain(
                        tree,
                        &old_chain,
                        &new_chain,
                        PseudoState::Hover,
                    );
                    tree.hovered_node = hit;
                    if let Some(old) = prev {
                        out_events.push(UiEvent::PointerLeave { node: old });
                    }
                    if let Some(new) = hit {
                        out_events.push(UiEvent::PointerEnter { node: new });
                    }
                }
            }
            UiInputEvent::PointerDown { pos, button: _ } => {
                let hit = Self::hit_test(paint_list, tree, pos);
                let old_chain = Self::ancestor_chain(tree, tree.active_node);
                let new_chain = Self::ancestor_chain(tree, hit);
                Self::update_pseudo_chain(tree, &old_chain, &new_chain, PseudoState::Active);
                tree.active_node = hit;
            }
            UiInputEvent::PointerUp { pos, button: _ } => {
                let hit = Self::hit_test(paint_list, tree, pos);
                let press_target = tree.active_node;
                let old_chain = Self::ancestor_chain(tree, press_target);
                Self::update_pseudo_chain(tree, &old_chain, &[], PseudoState::Active);
                tree.active_node = None;
                if let (Some(hit_id), Some(press_id)) = (hit, press_target) {
                    if hit_id == press_id {
                        out_events.push(UiEvent::Click { node: hit_id });
                        let is_focusable = tree
                            .nodes
                            .get(&hit_id)
                            .map(|n| n.attributes.contains_key("tabindex"))
                            .unwrap_or(false);
                        if is_focusable {
                            let prev_focus = tree.focused_node;
                            if prev_focus != Some(hit_id) {
                                if let Some(old) = prev_focus {
                                    if let Some(n) = tree.nodes.get_mut(&old) {
                                        n.state.pseudo_states.remove(&PseudoState::Focus);
                                        n.dirty.insert(DirtyFlag::Style);
                                    }
                                    out_events.push(UiEvent::FocusLost { node: old });
                                }
                                tree.focused_node = Some(hit_id);
                                if let Some(n) = tree.nodes.get_mut(&hit_id) {
                                    n.state.pseudo_states.insert(PseudoState::Focus);
                                    n.dirty.insert(DirtyFlag::Style);
                                }
                                out_events.push(UiEvent::FocusGained { node: hit_id });
                            }
                        }
                    }
                }
            }
            UiInputEvent::KeyDown { key: _ }
            | UiInputEvent::KeyUp { key: _ }
            | UiInputEvent::TextInput { .. }
            | UiInputEvent::Scroll { .. } => {}
        }
        Ok(())
    }

    /// Return the target node and each of its ancestors, in that order.
    /// `None` targets produce an empty vector. The chain matches CSS `:hover`
    /// / `:active` semantics: a pseudo-state on the topmost node also applies
    /// to every ancestor.
    pub fn ancestor_chain(tree: &UiTree, target: Option<NodeId>) -> Vec<NodeId> {
        let mut chain = Vec::new();
        let mut cur = target;
        while let Some(id) = cur {
            chain.push(id);
            cur = tree.nodes.get(&id).and_then(|n| n.parent);
        }
        chain
    }

    /// Diff two ancestor chains and toggle `state` accordingly: clear it on
    /// nodes only in `old_chain`, set it on nodes only in `new_chain`. Marks
    /// each changed node `Style`-dirty.
    fn update_pseudo_chain(
        tree: &mut UiTree,
        old_chain: &[NodeId],
        new_chain: &[NodeId],
        state: PseudoState,
    ) {
        for &id in old_chain {
            if new_chain.contains(&id) {
                continue;
            }
            if let Some(n) = tree.nodes.get_mut(&id) {
                if n.state.pseudo_states.remove(&state) {
                    n.dirty.insert(DirtyFlag::Style);
                }
            }
        }
        for &id in new_chain {
            if old_chain.contains(&id) {
                continue;
            }
            if let Some(n) = tree.nodes.get_mut(&id) {
                if n.state.pseudo_states.insert(state) {
                    n.dirty.insert(DirtyFlag::Style);
                }
            }
        }
    }

    /// Walk the paint list in reverse (last drawn = topmost) and return the
    /// `NodeId` of the first `Rect` whose bounds contain `pos` and whose node
    /// has `pointer-events` enabled.
    pub fn hit_test(paint_list: &PaintList, tree: &UiTree, pos: PointerPos) -> Option<NodeId> {
        for cmd in paint_list.commands.iter().rev() {
            if let PaintCommand::Rect {
                node_id, bounds, ..
            } = cmd
            {
                let pointer_events = tree
                    .nodes
                    .get(node_id)
                    .map(|n| n.computed_style.pointer_events)
                    .unwrap_or(false);
                if !pointer_events {
                    continue;
                }
                if pos.x >= bounds.x
                    && pos.x <= bounds.x + bounds.width
                    && pos.y >= bounds.y
                    && pos.y <= bounds.y + bounds.height
                {
                    return Some(*node_id);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::ui::{
        layout::LayoutBox,
        paint::PaintCommand,
        runtime::{UiNode, UiNodeState, UiTag},
        style::ComputedStyle,
    };

    fn make_node(id: NodeId, x: f32, y: f32, w: f32, h: f32, pointer_events: bool) -> UiNode {
        let mut style = ComputedStyle::default();
        style.pointer_events = pointer_events;
        UiNode {
            id,
            parent: None,
            children: vec![],
            tag: UiTag::UI,
            id_attr: None,
            class_list: vec![],
            attributes: HashMap::new(),
            inline_style: None,
            state: UiNodeState::default(),
            computed_style: style,
            layout: LayoutBox {
                x,
                y,
                width: w,
                height: h,
                content_x: x,
                content_y: y,
                content_width: w,
                content_height: h,
            },
            dirty: Default::default(),
        }
    }

    fn rect_cmd(node_id: NodeId, x: f32, y: f32, w: f32, h: f32) -> PaintCommand {
        PaintCommand::Rect {
            node_id,
            bounds: LayoutBox {
                x,
                y,
                width: w,
                height: h,
                content_x: x,
                content_y: y,
                content_width: w,
                content_height: h,
            },
            bg: crate::ui::colors::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            border_color: crate::ui::colors::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            border_width: 0.0,
            corner_radius: 0.0,
            clip: None,
        }
    }

    fn make_tree(nodes: Vec<UiNode>) -> UiTree {
        let root = nodes[0].id;
        let mut tree = UiTree::new();
        tree.root = root;
        for n in nodes {
            tree.nodes.insert(n.id, n);
        }
        tree
    }

    /// Three siblings painted in order (A, B, C). Clicking inside the overlap
    /// zone must return C — the last drawn (topmost) node.
    #[test]
    fn hit_test_picks_last_drawn_in_overlap() {
        let a = make_node(1, 0.0, 0.0, 100.0, 100.0, true);
        let b = make_node(2, 10.0, 10.0, 100.0, 100.0, true);
        let c = make_node(3, 20.0, 20.0, 100.0, 100.0, true);
        let tree = make_tree(vec![a, b, c]);

        let paint_list = PaintList {
            commands: vec![
                rect_cmd(1, 0.0, 0.0, 100.0, 100.0),
                rect_cmd(2, 10.0, 10.0, 100.0, 100.0),
                rect_cmd(3, 20.0, 20.0, 100.0, 100.0),
            ],
        };

        let hit = UiEventRouter::hit_test(&paint_list, &tree, PointerPos { x: 50.0, y: 50.0 });
        assert_eq!(hit, Some(3), "last drawn node should be hit first");
    }

    /// Moving the pointer into a node emits PointerEnter and marks Style dirty.
    /// Moving out emits PointerLeave and clears the Hover pseudo-state.
    #[test]
    fn hover_marks_style_on_entered_and_left() {
        let a = make_node(1, 0.0, 0.0, 50.0, 50.0, true);
        let b = make_node(2, 60.0, 0.0, 50.0, 50.0, true);
        let mut tree = make_tree(vec![a, b]);
        let paint_list = PaintList {
            commands: vec![
                rect_cmd(1, 0.0, 0.0, 50.0, 50.0),
                rect_cmd(2, 60.0, 0.0, 50.0, 50.0),
            ],
        };

        let mut events = vec![];
        // Move into node A
        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 25.0, y: 25.0 }),
            &mut events,
        )
        .unwrap();
        assert!(matches!(
            events.last(),
            Some(UiEvent::PointerEnter { node: 1 })
        ));
        assert!(tree.nodes[&1].dirty.contains(&DirtyFlag::Style));
        assert!(
            tree.nodes[&1]
                .state
                .pseudo_states
                .contains(&PseudoState::Hover)
        );

        // Move into node B (leaves A)
        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 85.0, y: 25.0 }),
            &mut events,
        )
        .unwrap();
        assert!(matches!(
            events[events.len() - 2],
            UiEvent::PointerLeave { node: 1 }
        ));
        assert!(matches!(
            events.last(),
            Some(UiEvent::PointerEnter { node: 2 })
        ));
        assert!(tree.nodes[&1].dirty.contains(&DirtyFlag::Style));
        assert!(
            !tree.nodes[&1]
                .state
                .pseudo_states
                .contains(&PseudoState::Hover)
        );
    }

    /// PointerDown then PointerUp on the same node emits Click.
    #[test]
    fn click_emitted_on_down_then_up_same_node() {
        let a = make_node(1, 0.0, 0.0, 100.0, 100.0, true);
        let mut tree = make_tree(vec![a]);
        let paint_list = PaintList {
            commands: vec![rect_cmd(1, 0.0, 0.0, 100.0, 100.0)],
        };
        let btn = winit::event::MouseButton::Left;

        let mut events = vec![];
        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerDown {
                pos: PointerPos { x: 50.0, y: 50.0 },
                button: btn,
            },
            &mut events,
        )
        .unwrap();
        assert!(
            tree.nodes[&1]
                .state
                .pseudo_states
                .contains(&PseudoState::Active)
        );

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerUp {
                pos: PointerPos { x: 50.0, y: 50.0 },
                button: btn,
            },
            &mut events,
        )
        .unwrap();
        assert!(matches!(events.last(), Some(UiEvent::Click { node: 1 })));
        assert!(
            !tree.nodes[&1]
                .state
                .pseudo_states
                .contains(&PseudoState::Active)
        );
        assert!(tree.active_node.is_none());
    }

    // ---------- ancestor propagation of :hover and :active ----------

    /// Build a `panel > button` tree that mirrors the panel/button layout
    /// in `examples/ui_panels`. Returns `(tree, panel_id, button_id, paint_list)`.
    /// The panel spans (0,0,200,200) and the button (50,50,100,100).
    fn nested_panel_button() -> (UiTree, NodeId, NodeId, PaintList) {
        let mut panel = make_node(1, 0.0, 0.0, 200.0, 200.0, true);
        panel.children = vec![2];
        let mut button = make_node(2, 50.0, 50.0, 100.0, 100.0, true);
        button.parent = Some(1);

        let tree = make_tree(vec![panel, button]);
        let paint_list = PaintList {
            commands: vec![
                rect_cmd(1, 0.0, 0.0, 200.0, 200.0),
                rect_cmd(2, 50.0, 50.0, 100.0, 100.0),
            ],
        };
        (tree, 1, 2, paint_list)
    }

    fn has_state(tree: &UiTree, id: NodeId, state: PseudoState) -> bool {
        tree.nodes[&id].state.pseudo_states.contains(&state)
    }

    /// Regression: hovering a nested button must also mark the parent panel
    /// as `:hover`. Previously only the topmost hit received the state.
    #[test]
    fn hover_over_child_propagates_to_parent() {
        let (mut tree, panel, button, paint_list) = nested_panel_button();
        let mut events = vec![];

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 100.0, y: 100.0 }),
            &mut events,
        )
        .unwrap();

        assert!(has_state(&tree, button, PseudoState::Hover));
        assert!(has_state(&tree, panel, PseudoState::Hover));
        assert_eq!(tree.hovered_node, Some(button));
    }

    /// Moving from the parent's empty area onto the child keeps the parent's
    /// hover state (both are hovered) and adds it to the child.
    #[test]
    fn moving_from_parent_body_onto_child_keeps_parent_hover() {
        let (mut tree, panel, button, paint_list) = nested_panel_button();
        let mut events = vec![];

        // Land on the panel body first (outside button bounds).
        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 10.0, y: 10.0 }),
            &mut events,
        )
        .unwrap();
        assert!(has_state(&tree, panel, PseudoState::Hover));
        assert!(!has_state(&tree, button, PseudoState::Hover));
        let dirty_before = tree.nodes[&panel].dirty.contains(&DirtyFlag::Style);
        assert!(dirty_before);
        tree.node_mut(panel).unwrap().dirty.clear();

        // Now move onto the child. Parent should stay hovered *without*
        // needing to re-emit a Style dirty flag, since its state didn't flip.
        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 100.0, y: 100.0 }),
            &mut events,
        )
        .unwrap();
        assert!(has_state(&tree, panel, PseudoState::Hover));
        assert!(has_state(&tree, button, PseudoState::Hover));
        assert!(
            !tree.nodes[&panel].dirty.contains(&DirtyFlag::Style),
            "parent should not be re-dirtied when its hover state does not change",
        );
    }

    /// Moving from the child back into the parent's body clears the child's
    /// hover state but preserves the parent's.
    #[test]
    fn moving_from_child_back_to_parent_body_only_clears_child() {
        let (mut tree, panel, button, paint_list) = nested_panel_button();
        let mut events = vec![];

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 100.0, y: 100.0 }),
            &mut events,
        )
        .unwrap();
        assert!(has_state(&tree, button, PseudoState::Hover));
        assert!(has_state(&tree, panel, PseudoState::Hover));

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 10.0, y: 10.0 }),
            &mut events,
        )
        .unwrap();
        assert!(!has_state(&tree, button, PseudoState::Hover));
        assert!(has_state(&tree, panel, PseudoState::Hover));
        assert_eq!(tree.hovered_node, Some(panel));
    }

    /// Moving the pointer entirely outside the panel clears hover on both
    /// child and parent.
    #[test]
    fn moving_pointer_off_tree_clears_hover_on_entire_chain() {
        let (mut tree, panel, button, paint_list) = nested_panel_button();
        let mut events = vec![];

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 100.0, y: 100.0 }),
            &mut events,
        )
        .unwrap();
        assert!(has_state(&tree, button, PseudoState::Hover));
        assert!(has_state(&tree, panel, PseudoState::Hover));

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 500.0, y: 500.0 }),
            &mut events,
        )
        .unwrap();
        assert!(!has_state(&tree, button, PseudoState::Hover));
        assert!(!has_state(&tree, panel, PseudoState::Hover));
        assert_eq!(tree.hovered_node, None);
    }

    /// Moving between sibling children of the same parent should only toggle
    /// hover on the children; the shared parent's state must stay stable.
    #[test]
    fn moving_between_siblings_keeps_shared_parent_hovered() {
        // parent(1) contains sibling_a(2) and sibling_b(3).
        let mut parent = make_node(1, 0.0, 0.0, 300.0, 100.0, true);
        parent.children = vec![2, 3];
        let mut a = make_node(2, 10.0, 10.0, 80.0, 80.0, true);
        a.parent = Some(1);
        let mut b = make_node(3, 200.0, 10.0, 80.0, 80.0, true);
        b.parent = Some(1);
        let mut tree = make_tree(vec![parent, a, b]);
        let paint_list = PaintList {
            commands: vec![
                rect_cmd(1, 0.0, 0.0, 300.0, 100.0),
                rect_cmd(2, 10.0, 10.0, 80.0, 80.0),
                rect_cmd(3, 200.0, 10.0, 80.0, 80.0),
            ],
        };
        let mut events = vec![];

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 50.0, y: 50.0 }),
            &mut events,
        )
        .unwrap();
        assert!(has_state(&tree, 2, PseudoState::Hover));
        assert!(has_state(&tree, 1, PseudoState::Hover));
        tree.node_mut(1).unwrap().dirty.clear();

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 240.0, y: 50.0 }),
            &mut events,
        )
        .unwrap();
        assert!(!has_state(&tree, 2, PseudoState::Hover));
        assert!(has_state(&tree, 3, PseudoState::Hover));
        assert!(has_state(&tree, 1, PseudoState::Hover));
        assert!(
            !tree.nodes[&1].dirty.contains(&DirtyFlag::Style),
            "shared parent should not be re-dirtied when its hover state does not change",
        );
    }

    /// Only PointerEnter/PointerLeave for the direct topmost node are emitted;
    /// ancestor state changes are silent to keep event streams predictable.
    #[test]
    fn hover_propagation_does_not_emit_extra_enter_leave_events() {
        let (mut tree, _panel, button, paint_list) = nested_panel_button();
        let mut events = vec![];

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerMove(PointerPos { x: 100.0, y: 100.0 }),
            &mut events,
        )
        .unwrap();

        let enters: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, UiEvent::PointerEnter { .. }))
            .collect();
        assert_eq!(enters.len(), 1);
        assert!(matches!(enters[0], UiEvent::PointerEnter { node } if *node == button));
    }

    /// Pressing on a nested child also marks the parent as `:active` so a
    /// parent's `:active` rule can respond to clicks originating in children.
    #[test]
    fn pointer_down_on_child_propagates_active_to_parent() {
        let (mut tree, panel, button, paint_list) = nested_panel_button();
        let mut events = vec![];
        let btn = winit::event::MouseButton::Left;

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerDown {
                pos: PointerPos { x: 100.0, y: 100.0 },
                button: btn,
            },
            &mut events,
        )
        .unwrap();
        assert!(has_state(&tree, button, PseudoState::Active));
        assert!(has_state(&tree, panel, PseudoState::Active));

        UiEventRouter::dispatch_input(
            &mut tree,
            &paint_list,
            UiInputEvent::PointerUp {
                pos: PointerPos { x: 100.0, y: 100.0 },
                button: btn,
            },
            &mut events,
        )
        .unwrap();
        assert!(!has_state(&tree, button, PseudoState::Active));
        assert!(!has_state(&tree, panel, PseudoState::Active));
    }

    /// The chain helper walks the full parent chain up to the root.
    #[test]
    fn ancestor_chain_walks_from_target_to_root() {
        let (tree, panel, button, _paint_list) = nested_panel_button();
        let chain = UiEventRouter::ancestor_chain(&tree, Some(button));
        assert_eq!(chain, vec![button, panel]);

        let empty = UiEventRouter::ancestor_chain(&tree, None);
        assert!(empty.is_empty());
    }
}
