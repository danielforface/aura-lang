#![cfg(feature = "lumina-diagnostics")]
// Real native X11/Xvfb + software GL framebuffer test.
// Asserts interior pixels and resize sampling; does not prove physical 2x
// monitor, real human input, pixel-perfect text or platform OS parity.

use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::{AuraLuminaPlugin,DiagnosticPixel};

fn node(kind:&str,props:&[(&str,&str)],children:Vec<UiNode>)->UiNode {
    let mut n=UiNode::new(kind);
    for &(k,v) in props { n.set_prop(k,v); }
    n.children=children;n
}
fn pixel(r:u8,g:u8,b:u8)->DiagnosticPixel { DiagnosticPixel{r,g,b,a:255} }

fn scene()->UiNode {
    node("App",&[("bg","#102030")],vec![
        node("Box",&[("width","80"),("height","80"),("overflow","hidden")],vec![
            node("Rect",&[("x","40"),("y","35"),("width","100"),
              ("height","40"),("color","#E02030")],vec![])
        ]),
        node("Rect",&[("x","160"),("y","20"),("width","40"),
          ("height","40"),("color","#00C878"),("radius","9")],vec![])
    ])
}
fn render(plugin:&AuraLuminaPlugin, tree:&UiNode, context:&mut NexusContext) {
    for _ in 0..4 {
        plugin.on_ui_render(tree,context)
            .expect("Lumina owns UI render")
            .expect("native Raylib render succeeded");
        let fb=context.get::<UiRuntimeFeedback>().expect("feedback");
        assert!(!fb.close_requested);
    }
}
fn sample(plugin:&AuraLuminaPlugin,x:f32,y:f32)->DiagnosticPixel {
    let p=plugin.diagnostic_capture_pixel(x,y).expect("valid framebuffer readback");
    eprintln!("LUMINA_PIXEL (x={x}, y={y}) = {p:?}");
    p
}

#[test]
fn actual_xvfb_resize_clipping_and_gpu_pixels_match_logical_layout() {
    let plugin=AuraLuminaPlugin::new();
    let mut context=NexusContext::default();
    let root=scene();
    render(&plugin,&root,&mut context);
    for (width,height) in [(640,420),(800,520),(420,310)] {
        assert!(plugin.diagnostic_resize_window(width,height));
        render(&plugin,&root,&mut context);
        let m=plugin.diagnostic_frame_metrics().expect("native window metrics");
        eprintln!("LUMINA_FRAME logical={}x{} render={}x{} scale={}x{}",
            m.logical_width,m.logical_height,m.render_width,m.render_height,
            m.scale_x,m.scale_y);
        assert_eq!((m.logical_width,m.logical_height),(width,height));
        assert_eq!((m.render_width,m.render_height),(width,height),
            "Xvfb 1x baseline only; high-DPI monitor is a different future gate");
        assert_eq!((m.scale_x,m.scale_y),(1.0,1.0));

        // Rect inside Box clip: x40..80, y35..75 is visible. x>=80 hidden.
        assert_eq!(sample(&plugin,60.0,50.0),pixel(224,32,48));
        assert_eq!(sample(&plugin,100.0,50.0),pixel(16,32,48));
        // Filled rounded rectangle uses gl_FragCoord: verify physical origin
        // and logical pixel conversion, away from antialiased edges.
        let rounded=sample(&plugin,180.0,40.0);
        assert_eq!(rounded,pixel(0,200,120));
        // Legacy Box has an opaque default white fill; (5,5) sits inside it.
        // Sample an unobstructed App pixel separately rather than assuming a
        // different Box theming contract in this framebuffer-specific gate.
        assert_eq!(sample(&plugin,5.0,5.0),pixel(255,255,255));
        assert_eq!(sample(&plugin,10.0,100.0),pixel(16,32,48));
        assert_eq!(sample(&plugin,(width-5) as f32,(height-5) as f32),pixel(16,32,48));
        assert_eq!(plugin.diagnostic_capture_pixel(width as f32,0.0),None);
    }
    drop(plugin);
    eprintln!("LUMINA_PIXEL_E2E: native window resize, framebuffer pixels and GPU teardown PASS");
}
