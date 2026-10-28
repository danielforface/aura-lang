#![cfg(feature = "raylib")]
// Native-window smoke only. Requires an actual X11 display (Xvfb in CI).
// It exercises Raylib initialization, shader setup, draw and scoped scissors.
// It is NOT a screenshot/pixel test, synthetic pointer test or DPI proof.

use aura_nexus::{AuraPlugin, NexusContext, UiNode, UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;

fn node(kind: &str, props: &[(&str, &str)], children: Vec<UiNode>) -> UiNode {
    let mut n = UiNode::new(kind);
    for (k, v) in props { n.set_prop(*k, *v); }
    n.children = children;
    n
}

#[test]
fn native_raylib_renders_scissored_nested_controls_and_publishes_feedback() {
    let mut context = NexusContext::default();
    let plugin = AuraLuminaPlugin::new();
    let tree = node("App", &[("bg", "#131C29")], vec![
        node("Box", &[("width","250"), ("height","90"), ("padding","12"),
            ("radius","8"), ("bg","#243447"), ("overflow","hidden")], vec![
            node("VStack", &[("spacing","8")], vec![
                node("Button", &[("width","180"), ("height","32"),
                    ("radius","6"), ("label","Click"), ("on_click","cb:11")], vec![]),
                node("TextInput", &[("width","150"), ("height","28"),
                    ("value","hello"), ("on_change","cb:12")], vec![])
            ])
        ]),
        node("Rect", &[("x","300"),("y","20"),("width","32"),
            ("height","32"),("radius","8"),("color","#729DFC")], vec![])
    ]);
    for frame in 0..3 {
        plugin.on_ui_render(&tree, &mut context)
            .expect("Lumina accepted UI tree")
            .expect("Raylib frame rendering must not report an error");
        let feedback = context.get::<UiRuntimeFeedback>().expect("frame feedback");
        assert!(!feedback.close_requested, "virtual window unexpectedly closed at frame {frame}");
        assert_eq!(feedback.clicked_callback_id, None);
        assert!(feedback.text_input_events.is_empty());
    }
}
