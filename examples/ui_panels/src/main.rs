use std::path::PathBuf;

use chaos_engine::ChaosMessageBuilder;
use chaos_engine::device::bindings::{
    ChaosBindingEvent, ChaosButton, ChaosDeviceEventMatcher, ChaosInputEventMatcher,
};
use chaos_engine::device::events::ChaosMouseButton;
use chaos_engine::engine::ChaosEngine;
use chaos_engine::log;
use chaos_engine::logger::ChaosLogger;
use chaos_engine::ui::layout::UiViewport;
use chaos_engine::ui::runtime::UiTemplate;
use chaos_engine::ui::system::UiSystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DeviceEvent {
    Resized,
    Input,
}

fn main() {
    log::set_max_level(log::LevelFilter::Debug);
    let _ = log::set_logger(&ChaosLogger {});

    let width = 1024u32;
    let height = 1024u32;

    let mut engine = ChaosEngine::new("UI Panels", width, height).expect("failed to init engine");

    // Point the engine's shader alias at the SPIR-V produced by build.rs.
    let shader_root = std::env::current_exe()
        .expect("current exe")
        .parent()
        .expect("exe parent")
        .join("res/internal_shaders");
    engine.add_directory(PathBuf::from("internal_shaders"), shader_root);

    // Wire window resize events so the UI system can react. The trigger must be
    // bound *before* the UI system registers for it during initialize().
    engine.device_event_system().bind(
        ChaosBindingEvent::Device(ChaosDeviceEventMatcher::Resized),
        DeviceEvent::Resized,
    );

    // Bind all pointer/keyboard events to a single Input trigger so the UI
    // system can do hover, click, and focus tracking.
    let input_bindings = [
        ChaosBindingEvent::Input(ChaosInputEventMatcher::MouseMoved),
        ChaosBindingEvent::Input(ChaosInputEventMatcher::Pressed(ChaosButton::Mouse(
            ChaosMouseButton::Left,
        ))),
        ChaosBindingEvent::Input(ChaosInputEventMatcher::Released(ChaosButton::Mouse(
            ChaosMouseButton::Left,
        ))),
        ChaosBindingEvent::Input(ChaosInputEventMatcher::MouseWheel),
    ];
    for binding in input_bindings {
        engine
            .device_event_system()
            .bind(binding, DeviceEvent::Input);
    }

    // Watch the source files under the crate's res/ui/ directory so hot-reload
    // picks up edits the developer actually makes.
    let ui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("res/ui");
    let markup_path = ui_dir.join("panels.ui");
    let css_path = ui_dir.join("panels.css");

    let markup = std::fs::read_to_string(&markup_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", markup_path.display()));
    let css = std::fs::read_to_string(&css_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", css_path.display()));

    let template = UiTemplate::from_src(&markup, &css).expect("parse ui template");
    let viewport = UiViewport {
        width: width as f32,
        height: height as f32,
        scale_factor: 1.0,
    };

    let mut ui_system =
        UiSystem::with_resize_trigger(template, viewport, Some(DeviceEvent::Resized))
            .with_input_trigger(DeviceEvent::Input)
            .with_default_font();

    #[cfg(debug_assertions)]
    if let Err(e) = ui_system.enable_hot_reload(markup_path.clone(), css_path.clone()) {
        log::warn!("hot-reload disabled: {e}");
    }

    ui_system.register_binding("toggle_pause", |_event| {
        log::info!("toggle_pause action fired");
        Some(
            ChaosMessageBuilder::new()
                .with_param("action", "toggle_pause".to_string())
                .build_for_event("toggle_pause"),
        )
    });

    engine.world_mut().add_render_system(ui_system);

    engine.run();
}

#[cfg(test)]
mod tests {
    use chaos_engine::ui::{
        layout::{UiLayoutEngine, UiViewport},
        paint::{PaintCommand, UiPainter},
        runtime::{DefaultUiTemplateInstantiator, UiTemplate, UiTemplateInstantiator},
    };

    fn load_template() -> UiTemplate {
        let markup = include_str!("../res/ui/panels.ui");
        let css = include_str!("../res/ui/panels.css");
        UiTemplate::from_src(markup, css).expect("parse panels template")
    }

    #[test]
    fn panels_template_contains_text_labels() {
        let template = load_template();
        let mut tree = DefaultUiTemplateInstantiator.instantiate(&template);
        tree.apply_style(&template.stylesheet).unwrap();

        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();
        engine
            .compute_layout(
                &mut tree,
                UiViewport {
                    width: 1024.0,
                    height: 768.0,
                    scale_factor: 1.0,
                },
            )
            .unwrap();

        let paint_list = UiPainter::build_paint_list(&tree);

        let texts: Vec<&str> = paint_list
            .commands
            .iter()
            .filter_map(|c| match c {
                PaintCommand::Text { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(
            texts.len(),
            5,
            "expected one Text command per panel plus the button"
        );
        assert!(texts.contains(&"Panel A"));
        assert!(texts.contains(&"Panel B"));
        assert!(texts.contains(&"Panel C"));
        assert!(texts.contains(&"Panel D"));
        assert!(texts.contains(&"Pause"));
    }

    #[test]
    fn text_nodes_have_positive_layout_width() {
        let template = load_template();
        let mut tree = DefaultUiTemplateInstantiator.instantiate(&template);
        tree.apply_style(&template.stylesheet).unwrap();

        let mut engine = UiLayoutEngine::new();
        engine.sync_structure(&mut tree).unwrap();
        engine
            .compute_layout(
                &mut tree,
                UiViewport {
                    width: 1024.0,
                    height: 768.0,
                    scale_factor: 1.0,
                },
            )
            .unwrap();

        // Every Text node (leaf with UiTag::Text) should have been measured and
        // given a positive width by the fallback estimator in compute_layout.
        use chaos_engine::ui::runtime::UiTag;
        let text_widths: Vec<f32> = tree
            .nodes
            .values()
            .filter_map(|n| match &n.tag {
                UiTag::Text(_) => Some(n.layout.width),
                _ => None,
            })
            .collect();

        assert_eq!(text_widths.len(), 5, "expected five Text nodes in the tree");
        for w in &text_widths {
            assert!(*w > 0.0, "text node width should be positive, got {w}");
        }
    }
}
