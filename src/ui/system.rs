use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};

use std::collections::HashMap;

use fontdue::Font;

use crate::ChaosReceiver;
use crate::device::events::ChaosInputEvent;
use crate::ecs::system::ChaosSystem;
use crate::ecs::world::ChaosWorld;
use crate::rendering::draw_command::ChaosDrawQueue;
use crate::rendering::renderer::ChaosRenderContext;
use crate::rendering::rendering_system::ChaosRenderSystem;
use crate::ui::events::{PointerPos, UiEvent, UiEventRouter, UiInputEvent};
use crate::ui::layout::{UiLayoutEngine, UiViewport};
use crate::ui::paint::{
    DEFAULT_UI_SHADER_ALIAS, PaintList, UiPainter, UiRenderBackend, VulkanUiBackend,
};
use crate::ui::runtime::{
    DefaultUiTemplateInstantiator, DirtyFlag, UiTemplate, UiTemplateInstantiator, UiTree,
};
use crate::ui::{FontId, default_font};

/// Owns the UI tree and drives its per-frame passes
/// (Structure → Style → Layout → Paint) plus GPU submission.
///
/// Register with `world.add_render_system(...)`. On construction supply the
/// [`UiTemplate`] and an initial [`UiViewport`]; a `resize_trigger` may be
/// supplied so the system rebuilds layout in response to window resizes emitted
/// by the device event system. The trigger value must match one registered via
/// `engine.device_event_system().bind(ChaosBindingEvent::Device(
/// ChaosDeviceEventMatcher::Resized), <trigger>)`.
pub struct UiSystem<R = ()>
where
    R: Hash + Copy + Send + Sync + 'static,
{
    template: UiTemplate,
    tree: UiTree,
    layout_engine: UiLayoutEngine,
    painter: UiPainter,
    paint_list: PaintList,
    event_queue: Vec<UiEvent>,
    viewport: UiViewport,
    resize_trigger: Option<R>,
    resize_receiver: Option<ChaosReceiver>,
    /// Trigger key whose receiver delivers all raw input messages for this system.
    /// Callers bind mouse-move, mouse-button, etc. to this trigger before registering
    /// the system; `initialize()` subscribes automatically.
    input_trigger: Option<R>,
    input_receiver: Option<ChaosReceiver>,
    /// Tracks the last known pointer position so button events can reconstruct it.
    current_pos: PointerPos,
    backend: Option<VulkanUiBackend>,
    shader_alias: String,
    paint_dirty: bool,
    hot_reload: Option<HotReloadState>,
    /// Fonts registered before GPU init; seeded into the backend and layout engine.
    fonts: HashMap<FontId, Font>,
}

/// State kept by [`UiSystem`] while hot-reload is active. The `_guard` keeps
/// the underlying filesystem watcher alive; dropping it stops watching. Type
/// erasure lets tests inject a channel without constructing a real debouncer.
pub struct HotReloadState {
    _guard: Box<dyn std::any::Any + Send>,
    rx: mpsc::Receiver<PathBuf>,
    markup_path: PathBuf,
    css_path: PathBuf,
}

impl HotReloadState {
    /// Test-only constructor: build a `HotReloadState` around a pre-existing
    /// channel receiver so tests can drive reloads without a real watcher.
    #[cfg(test)]
    fn from_channel(rx: mpsc::Receiver<PathBuf>, markup_path: PathBuf, css_path: PathBuf) -> Self {
        Self {
            _guard: Box::new(()),
            rx,
            markup_path,
            css_path,
        }
    }
}

impl UiSystem<()> {
    /// Convenience constructor for callers that don't wire a resize trigger.
    pub fn new(template: UiTemplate, initial_viewport: UiViewport) -> Self {
        Self::with_resize_trigger(template, initial_viewport, None::<()>)
    }
}

impl<R> UiSystem<R>
where
    R: Hash + Copy + Send + Sync + 'static,
{
    pub fn with_resize_trigger(
        template: UiTemplate,
        initial_viewport: UiViewport,
        resize_trigger: Option<R>,
    ) -> Self {
        let tree = DefaultUiTemplateInstantiator.instantiate(&template);
        Self {
            template,
            tree,
            layout_engine: UiLayoutEngine::new(),
            painter: UiPainter,
            paint_list: PaintList::default(),
            event_queue: Vec::new(),
            viewport: initial_viewport,
            resize_trigger,
            resize_receiver: None,
            input_trigger: None,
            input_receiver: None,
            current_pos: PointerPos { x: 0.0, y: 0.0 },
            backend: None,
            shader_alias: DEFAULT_UI_SHADER_ALIAS.to_string(),
            paint_dirty: true,
            hot_reload: None,
            fonts: HashMap::new(),
        }
    }

    /// Register a font for text layout and rendering. The font is immediately
    /// seeded into the layout engine; it is also passed to the GPU backend when
    /// `initialize_rendering` runs.
    pub fn load_font(&mut self, name: &str, bytes: &[u8]) -> Result<FontId, String> {
        let font = Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| format!("failed to parse font '{name}': {e}"))?;
        let id = self.fonts.len() as FontId;
        self.layout_engine.add_font(id, font.clone());
        self.fonts.insert(id, font);
        Ok(id)
    }

    /// Load the bundled VT323 Regular font as the default (id 0).
    pub fn with_default_font(mut self) -> Self {
        if let Err(e) = self.load_font("default", default_font::VT323_REGULAR) {
            log::warn!("failed to load default font: {e}");
        }
        self
    }

    /// Set the trigger whose receiver delivers all raw input messages. Bind
    /// mouse-move, mouse-button, scroll, etc. to this trigger via
    /// `engine.device_event_system().bind(...)` before running the engine.
    pub fn with_input_trigger(mut self, trigger: R) -> Self {
        self.input_trigger = Some(trigger);
        self
    }

    /// Override the shader alias key used to look up `ui_rect` shaders. Must
    /// match a directory registered via `ChaosEngine::add_directory(alias,
    /// ...)`. Defaults to [`DEFAULT_UI_SHADER_ALIAS`].
    pub fn with_shader_alias(mut self, alias: impl Into<String>) -> Self {
        self.shader_alias = alias.into();
        self
    }

    pub fn tree(&self) -> &UiTree {
        &self.tree
    }

    pub fn paint_list(&self) -> &PaintList {
        &self.paint_list
    }

    pub fn viewport(&self) -> UiViewport {
        self.viewport
    }

    /// Consume any pending UI events queued for downstream systems.
    pub fn drain_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.event_queue)
    }

    /// Start watching the given markup + stylesheet files. Whenever either
    /// changes on disk the next `update` re-reads and re-parses, swaps the
    /// template, and forces a full Structure+Style pass so layout and paint
    /// pick up the change.
    ///
    /// Errors from the underlying filesystem watcher are returned as strings.
    /// Parse errors happen later during `update` and are logged as warnings —
    /// the previous template is retained.
    pub fn enable_hot_reload(
        &mut self,
        markup_path: PathBuf,
        css_path: PathBuf,
    ) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(
            Duration::from_millis(100),
            move |res: DebounceEventResult| match res {
                Ok(events) => {
                    for e in events {
                        let _ = tx.send(e.path);
                    }
                }
                Err(err) => {
                    log::warn!("ui hot-reload watcher error: {err}");
                }
            },
        )
        .map_err(|e| format!("hot-reload debouncer init failed: {e}"))?;
        debouncer
            .watcher()
            .watch(&markup_path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("watch {}: {e}", markup_path.display()))?;
        debouncer
            .watcher()
            .watch(&css_path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("watch {}: {e}", css_path.display()))?;
        self.hot_reload = Some(HotReloadState {
            _guard: Box::new(debouncer),
            rx,
            markup_path,
            css_path,
        });
        Ok(())
    }

    pub fn is_hot_reload_enabled(&self) -> bool {
        self.hot_reload.is_some()
    }

    fn drain_hot_reload(&mut self) {
        let Some(hr) = self.hot_reload.as_mut() else {
            return;
        };
        let mut got_change = false;
        while let Ok(_path) = hr.rx.try_recv() {
            got_change = true;
        }
        if !got_change {
            return;
        }
        let markup_path = hr.markup_path.clone();
        let css_path = hr.css_path.clone();
        self.reload_from_paths(&markup_path, &css_path);
    }

    /// Re-read both files, re-parse, and swap the template. On any error the
    /// previous template is retained and a warning is logged.
    fn reload_from_paths(&mut self, markup_path: &PathBuf, css_path: &PathBuf) {
        let markup = match std::fs::read_to_string(markup_path) {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "ui hot-reload: failed to read markup {}: {e}",
                    markup_path.display()
                );
                return;
            }
        };
        let css = match std::fs::read_to_string(css_path) {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "ui hot-reload: failed to read css {}: {e}",
                    css_path.display()
                );
                return;
            }
        };
        self.apply_reload_from_str(&markup, &css);
    }

    /// Parse the given sources and, on success, swap in the new template,
    /// rebuild the tree, and mark the full tree Style/Structure/Layout dirty so
    /// the next passes rebuild layout and paint. On parse failure the previous
    /// template is retained.
    fn apply_reload_from_str(&mut self, markup: &str, css: &str) {
        let new_template = match UiTemplate::from_src(markup, css) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("ui hot-reload: parse failed ({e}); keeping previous template");
                return;
            }
        };
        self.template = new_template;
        // Markup changes can add/remove nodes we can't diff, so rebuild the
        // tree and layout engine from scratch rather than trying to patch them.
        self.tree = DefaultUiTemplateInstantiator.instantiate(&self.template);
        self.layout_engine = UiLayoutEngine::new();
        if let Some(root) = self.tree.nodes.get_mut(&self.tree.root) {
            root.dirty.insert(DirtyFlag::Structure);
            root.dirty.insert(DirtyFlag::Layout);
        }
        for n in self.tree.nodes.values_mut() {
            n.dirty.insert(DirtyFlag::Style);
        }
        self.paint_dirty = true;
        log::info!("ui hot-reload: applied");
    }

    /// Attach a receiver that supplies raw `ChaosInputEvent` messages (via the
    /// `"input_event"` field). Callers should bind all relevant input events
    /// (mouse position, buttons, scroll, keys) to the same trigger key and pass
    /// the resulting receiver here.
    pub fn set_input_receiver(&mut self, receiver: ChaosReceiver) {
        self.input_receiver = Some(receiver);
    }

    fn drain_input_events(&mut self) {
        let Some(receiver) = self.input_receiver.as_mut() else {
            return;
        };
        let mut inputs: Vec<UiInputEvent> = Vec::new();
        while let Some(msg) = receiver.receive() {
            if let Some(event) = msg.get::<ChaosInputEvent>("input_event") {
                if let Some(ui) = map_chaos_input(event, &mut self.current_pos) {
                    inputs.push(ui);
                }
            }
        }
        for input in inputs {
            let paint_list = &self.paint_list;
            let tree = &mut self.tree;
            let queue = &mut self.event_queue;
            if let Err(e) = UiEventRouter::dispatch_input(tree, paint_list, input, queue) {
                log::warn!("UiSystem dispatch_input: {e}");
            }
        }
    }

    fn drain_resize_events(&mut self) {
        let Some(receiver) = self.resize_receiver.as_mut() else {
            return;
        };
        let mut latest: Option<(u32, u32)> = None;
        while let Some(message) = receiver.receive() {
            if let (Some(w), Some(h)) = (message.get::<u32>("width"), message.get::<u32>("height"))
            {
                latest = Some((w, h));
            }
        }
        if let Some((w, h)) = latest {
            self.viewport = UiViewport {
                width: w as f32,
                height: h as f32,
                scale_factor: self.viewport.scale_factor,
            };
            if let Some(root) = self.tree.nodes.get_mut(&self.tree.root) {
                root.dirty.insert(DirtyFlag::Layout);
            }
        }
    }

    fn run_passes(&mut self) -> Result<(), String> {
        self.layout_engine.sync_structure(&mut self.tree)?;
        self.tree.apply_style(&self.template.stylesheet)?;
        self.layout_engine.sync_style(&self.tree)?;
        let any_layout_dirty = self
            .tree
            .nodes
            .values()
            .any(|n| n.dirty.contains(&DirtyFlag::Layout));
        if any_layout_dirty {
            self.layout_engine
                .compute_layout(&mut self.tree, self.viewport)?;
        }
        let any_paint_dirty = self
            .tree
            .nodes
            .values()
            .any(|n| n.dirty.contains(&DirtyFlag::Paint));
        if any_paint_dirty || any_layout_dirty {
            self.paint_list = UiPainter::build_paint_list(&self.tree);
            self.paint_dirty = true;
            for n in self.tree.nodes.values_mut() {
                n.dirty.remove(&DirtyFlag::Paint);
            }
        }
        Ok(())
    }
}

/// Convert a raw engine input event to the UI input domain, updating the
/// tracked pointer position for events that don't carry their own position.
fn map_chaos_input(event: ChaosInputEvent, pos: &mut PointerPos) -> Option<UiInputEvent> {
    match event {
        ChaosInputEvent::MousePosition { x, y } => {
            pos.x = x as f32;
            pos.y = y as f32;
            Some(UiInputEvent::PointerMove(*pos))
        }
        ChaosInputEvent::MouseButton {
            button,
            pressed: true,
        } => Some(UiInputEvent::PointerDown { pos: *pos, button }),
        ChaosInputEvent::MouseButton {
            button,
            pressed: false,
        } => Some(UiInputEvent::PointerUp { pos: *pos, button }),
        ChaosInputEvent::KeyboardInput {
            keycode: key,
            pressed: true,
        } => Some(UiInputEvent::KeyDown { key }),
        ChaosInputEvent::KeyboardInput {
            keycode: key,
            pressed: false,
        } => Some(UiInputEvent::KeyUp { key }),
        ChaosInputEvent::MouseWheel {
            delta_x: dx,
            delta_y: dy,
        } => Some(UiInputEvent::Scroll { dx, dy }),
    }
}

impl<R> ChaosSystem for UiSystem<R>
where
    R: Hash + Copy + Send + Sync + 'static,
{
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        if let Some(trigger) = self.resize_trigger {
            self.resize_receiver = Some(world.register_for_trigger(trigger));
        }
        if let Some(trigger) = self.input_trigger {
            self.input_receiver = Some(world.register_for_trigger(trigger));
        }
        Ok(())
    }

    fn update(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        self.drain_hot_reload();
        self.drain_resize_events();
        self.drain_input_events();
        self.run_passes().map_err(|e| {
            log::error!("UiSystem update failed: {e}");
            "ui system update failed"
        })
    }
}

impl<R> ChaosRenderSystem for UiSystem<R>
where
    R: Hash + Copy + Send + Sync + 'static,
{
    fn initialize_rendering(
        &mut self,
        _world: &mut ChaosWorld,
        ctx: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        if self.viewport.width <= 0.0 || self.viewport.height <= 0.0 {
            let vp = ctx.viewport();
            self.viewport = UiViewport {
                width: vp.extent[0].max(1.0),
                height: vp.extent[1].max(1.0),
                scale_factor: if self.viewport.scale_factor > 0.0 {
                    self.viewport.scale_factor
                } else {
                    1.0
                },
            };
        }
        let mut backend =
            VulkanUiBackend::with_shader_alias(ctx, &self.shader_alias).map_err(|e| {
                log::error!("UiSystem failed to initialize backend: {e}");
                "ui system failed to initialize backend"
            })?;

        if let Err(e) = backend.enable_text(ctx) {
            log::warn!("UiSystem text pipeline unavailable: {e}");
        } else {
            for (&id, font) in &self.fonts {
                backend.add_font(id, font.clone());
            }
        }

        self.backend = Some(backend);

        if let Some(root) = self.tree.nodes.get_mut(&self.tree.root) {
            root.dirty.insert(DirtyFlag::Layout);
        }
        self.run_passes().map_err(|e| {
            log::error!("UiSystem initial pass failed: {e}");
            "ui system initial pass failed"
        })
    }

    fn prepare_rendering(
        &mut self,
        _world: &mut ChaosWorld,
        draw_queue: &mut ChaosDrawQueue,
        ctx: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let Some(backend) = self.backend.as_mut() else {
            return Ok(());
        };
        if self.paint_dirty {
            backend
                .upload_resources(ctx, &self.paint_list, self.viewport)
                .map_err(|e| {
                    log::error!("UiSystem upload_resources failed: {e}");
                    "ui system upload_resources failed"
                })?;
            self.paint_dirty = false;
        }
        backend
            .submit_paint_list(&self.paint_list, draw_queue)
            .map_err(|e| {
                log::error!("UiSystem submit_paint_list failed: {e}");
                "ui system submit_paint_list failed"
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::runtime::UiTemplate;

    const MARKUP_A: &str = r#"<ui id="root"><ui class="a"/></ui>"#;
    const CSS_A: &str = ".a { background-color: #ff0000; }";

    const MARKUP_B: &str = r#"<ui id="root"><ui class="a"/><ui class="b"/></ui>"#;
    const CSS_B: &str = ".a { background-color: #00ff00; } .b { background-color: #0000ff; }";

    fn make_system() -> UiSystem<()> {
        let template = UiTemplate::from_src(MARKUP_A, CSS_A).expect("parse initial");
        let viewport = UiViewport {
            width: 100.0,
            height: 100.0,
            scale_factor: 1.0,
        };
        UiSystem::new(template, viewport)
    }

    #[test]
    fn apply_reload_from_str_swaps_template_and_marks_dirty() {
        let mut sys = make_system();
        let initial_nodes = sys.tree.nodes.len();

        sys.apply_reload_from_str(MARKUP_B, CSS_B);

        assert!(
            sys.tree.nodes.len() > initial_nodes,
            "new template added a child, tree should have grown"
        );
        let root = sys.tree.nodes.get(&sys.tree.root).expect("root exists");
        assert!(root.dirty.contains(&DirtyFlag::Structure));
        assert!(root.dirty.contains(&DirtyFlag::Layout));
        assert!(
            sys.tree
                .nodes
                .values()
                .all(|n| n.dirty.contains(&DirtyFlag::Style)),
            "every node should be Style-dirty after reload"
        );
        assert!(sys.paint_dirty);
    }

    #[test]
    fn apply_reload_from_str_preserves_previous_template_on_parse_error() {
        let mut sys = make_system();
        let before_count = sys.tree.nodes.len();

        // Malformed CSS should be rejected; tree stays intact.
        sys.apply_reload_from_str(MARKUP_A, "this is { not valid");

        assert_eq!(
            sys.tree.nodes.len(),
            before_count,
            "tree should not have been rebuilt on parse failure"
        );
    }

    #[test]
    fn update_drains_hot_reload_channel_and_reloads_from_files() {
        let dir = std::env::temp_dir().join(format!("chaos_ui_hot_reload_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mk tmp dir");
        let markup_path = dir.join("panel.ui");
        let css_path = dir.join("panel.css");
        std::fs::write(&markup_path, MARKUP_B).expect("write markup");
        std::fs::write(&css_path, CSS_B).expect("write css");

        let mut sys = make_system();
        let (tx, rx) = mpsc::channel::<PathBuf>();
        sys.hot_reload = Some(HotReloadState::from_channel(
            rx,
            markup_path.clone(),
            css_path.clone(),
        ));

        tx.send(markup_path.clone()).expect("send path");

        sys.drain_hot_reload();

        let root = sys.tree.nodes.get(&sys.tree.root).expect("root exists");
        assert!(root.dirty.contains(&DirtyFlag::Structure));
        // The new template's root has two children.
        assert_eq!(root.children.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }
}
