#![cfg(feature = "lumina-diagnostics")]
// REAL native X11 window plus system DejaVu TTF, but this deliberately
// verifies *shaping analysis*, not shaped glyph-ID GPU rasterization.
use aura_nexus::{AuraPlugin, NexusContext, UiNode};
use aura_plugin_lumina::AuraLuminaPlugin;
const FONT:&str="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
#[test]
fn native_bidi_and_arabic_gsub_are_not_naive_string_reverse_or_unicode_glyphs() {
    assert!(std::path::Path::new(FONT).is_file(),"font package must be installed");
    let p=AuraLuminaPlugin::new();
    let mut cx=NexusContext::default();
    let mut app=UiNode::new("App");
    let mut text=UiNode::new("Text");
    text.set_prop("text","ABC");
    text.set_prop("font_path",FONT);
    app.children.push(text);
    for _ in 0..5 {p.on_ui_render(&app,&mut cx).expect("runtime").expect("native draw");}
    let mixed="שלום ABC 123 עולם";
    let r=p.diagnostic_shape_external_font(FONT,mixed,24,"auto").expect("UAX9 + HarfBuzz-port shaping");
    eprintln!("LUMINA_SHAPE_X11 mixed runs={:?} rtl={:?} glyphs={} width={}",
        r.visual_source_starts,r.visual_run_rtl,r.glyph_ids.len(),r.width);
    assert!(!r.native_paintable, "no glyph-ID GPU renderer exists; no false claim");
    assert!(r.width.is_finite() && r.width>20.0);
    assert!(r.visual_source_starts.len()>=2 &&
        r.visual_run_rtl.iter().any(|rtl|*rtl) &&
        r.visual_run_rtl.iter().any(|rtl|!*rtl));
    assert!(r.visual_source_starts[0]>0,"RTL paragraph reorders visual runs");
    assert!(r.logical_cluster_bytes.iter().all(|b|mixed.is_char_boundary(*b)));
    assert!(r.glyph_ids.iter().all(|g|*g!=0));
    let bytes=std::fs::read(FONT).expect("TTF bytes");
    let face=rustybuzz::ttf_parser::Face::parse(&bytes,0).expect("valid OpenType");
    let arabic="سلام";
    let r=p.diagnostic_shape_external_font(FONT,arabic,32,"rtl").expect("Arabic joining font");
    let nominal:Vec<u16>=arabic.chars().map(|c|face.glyph_index(c).expect("Arabic glyph").0).collect();
    let materially_shaped=r.glyph_ids.len()!=nominal.len() ||
        r.glyph_ids.iter().any(|id|!nominal.contains(id));
    eprintln!("LUMINA_SHAPE_X11 Arabic shaped ids={:?} nominal={:?}",r.glyph_ids,nominal);
    assert!(materially_shaped,
        "Arabic OpenType GSUB must change glyph IDs or ligature count, not merely reverse scalars");
    assert!(!r.native_paintable);
    assert!(p.diagnostic_shape_external_font(FONT,arabic,32,"sideways").is_none());
    assert!(p.diagnostic_shape_external_font(FONT,"A\u{202E}B",32,"auto").is_none());
    assert!(p.diagnostic_shape_external_font(FONT,&"a".repeat(65537),32,"auto").is_none());
    drop(p);
}
