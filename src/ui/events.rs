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
                    if let Some(old) = prev {
                        if let Some(n) = tree.nodes.get_mut(&old) {
                            n.state.pseudo_states.remove(&PseudoState::Hover);
                            n.dirty.insert(DirtyFlag::Style);
                        }
                        out_events.push(UiEvent::PointerLeave { node: old });
                    }
                    tree.hovered_node = hit;
                    if let Some(new) = hit {
                        if let Some(n) = tree.nodes.get_mut(&new) {
                            n.state.pseudo_states.insert(PseudoState::Hover);
                            n.dirty.insert(DirtyFlag::Style);
                        }
                        out_events.push(UiEvent::PointerEnter { node: new });
                    }
                }
            }
            UiInputEvent::PointerDown { pos, button: _ } => {
                let hit = Self::hit_test(paint_list, tree, pos);
                if let Some(old) = tree.active_node {
                    if let Some(n) = tree.nodes.get_mut(&old) {
                        n.state.pseudo_states.remove(&PseudoState::Active);
                        n.dirty.insert(DirtyFlag::Style);
                    }
                }
                tree.active_node = hit;
                if let Some(new) = hit {
                    if let Some(n) = tree.nodes.get_mut(&new) {
                        n.state.pseudo_states.insert(PseudoState::Active);
                        n.dirty.insert(DirtyFlag::Style);
                    }
                }
            }
            UiInputEvent::PointerUp { pos, button: _ } => {
                let hit = Self::hit_test(paint_list, tree, pos);
                let press_target = tree.active_node;
                if let Some(old) = press_target {
                    if let Some(n) = tree.nodes.get_mut(&old) {
                        n.state.pseudo_states.remove(&PseudoState::Active);
                        n.dirty.insert(DirtyFlag::Style);
                    }
                }
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
}
