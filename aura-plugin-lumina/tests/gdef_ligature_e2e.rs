#![cfg(feature="lumina-diagnostics")]
// Linux Xvfb, real OS font installed as CI package fonts-linuxlibertine.
// A true GSUB "ffi" one-glyph ligature exposes two GDEF internal carets at
// 266 and 550 font units; native XTest cursor/click MUST use these units.
use aura_nexus::{AuraPlugin,NexusContext,UiNode,UiRuntimeFeedback};
use aura_plugin_lumina::AuraLuminaPlugin;
use std::process::Command;
const FONT:&str="/usr/share/fonts/opentype/linux-libertine/LinLibertine_R.otf";
fn node(kind:&str,props:&[(&str,&str)])->UiNode{
    let mut n=UiNode::new(kind);for &(k,v) in props{n.set_prop(k,v);}n
}
fn frames(p:&AuraLuminaPlugin,tree:&UiNode,c:&mut NexusContext,n:usize){
    for _ in 0..n{
        p.on_ui_render(tree,c).expect("native owned").expect("native draw");
        assert!(!c.get::<UiRuntimeFeedback>().unwrap().close_requested);
    }
}
fn xdo(args:&[&str])->String{
    let out=Command::new("xdotool").args(args).output().unwrap();
    assert!(out.status.success(),"{args:?}: {}",String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
fn key(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,k:&str){
    xdo(&["keydown",k]);frames(p,t,c,5);
    xdo(&["keyup",k]);frames(p,t,c,3);
}
fn click(p:&AuraLuminaPlugin,t:&UiNode,c:&mut NexusContext,id:&str,x:i32){
    xdo(&["mousemove","--window",id,&x.to_string(),"51"]);frames(p,t,c,3);
    xdo(&["mousedown","1"]);frames(p,t,c,5);
    xdo(&["mouseup","1"]);frames(p,t,c,3);
}
#[test]fn authentic_linux_libertine_native_gdef_ffi_caret_and_mouse_hit(){
    assert!(std::path::Path::new(FONT).is_file(),
        "CI must install fonts-linuxlibertine, no silently skipped native proof");
    let p=AuraLuminaPlugin::new();let mut c=NexusContext::default();
    let mut app=node("App",&[("bg","#101820")]);
    app.children.push(node("TextInput",&[
        ("x","20"),("y","20"),("width","330"),("height","64"),
        ("radius","0"),("value","ffi"),("value_revision","1"),
        ("on_change","cb:915"),("size","30"),("fg","#EE3030"),
        ("font_path",FONT),("shaping","opentype"),("direction","ltr")
    ]));
    frames(&p,&app,&mut c,4);
    assert!(p.diagnostic_resize_window(640,420));frames(&p,&app,&mut c,4);
    let (cells,glyphs,width)=p.diagnostic_shaped_gpu(FONT,30,"ltr","ffi")
       .expect("real Linux Libertine indexed GPU atlas");
    eprintln!("GDEF_NATIVE_GLYPH {glyphs:?} cells={cells} width={width}");
    assert_eq!(glyphs,vec![2649],"GSUB must be a single real ffi glyph");
    assert_eq!(cells,1);
    let paint=p.diagnostic_red_signature(30,25,160,63).unwrap();
    assert!(paint.0>50,"actual GPU red ligature pixels required: {paint:?}");
    let ids=xdo(&["search","--name","Aura Lumina Sentinel"]);
    let id=ids.lines().next().expect("real X11 window");
    xdo(&["windowfocus","--sync",id]);
    key(&p,&app,&mut c,"Tab");
    key(&p,&app,&mut c,"Home");
    let home=p.diagnostic_text_visual(915,32.0,306.0).unwrap();
    assert!(home.caret_visible);
    assert_eq!(home.caret_grapheme,Some(0));
    key(&p,&app,&mut c,"Right");
    let first=p.diagnostic_text_visual(915,32.0,306.0).unwrap();
    assert!(first.caret_visible);
    assert_eq!(first.caret_grapheme,Some(1));
    assert!((first.caret_screen_x-home.caret_screen_x-7.98).abs()<0.2,
        "first native caret must use GDEF coordinate 266 units at 30px");
    key(&p,&app,&mut c,"Right");
    let second=p.diagnostic_text_visual(915,32.0,306.0).unwrap();
    assert_eq!(second.caret_grapheme,Some(2));
    assert!((second.caret_screen_x-home.caret_screen_x-16.5).abs()<0.2,
        "second native caret must use GDEF coordinate 550 units at 30px");
    assert!(second.caret_screen_x>first.caret_screen_x+1.0);
    eprintln!("GDEF_NATIVE_XTEST home={home:?} first={first:?} second={second:?}");
    // Clicking near the first actual ligature caret must select the logical
    // grapheme boundary, despite one GPU-drawn glyph.
    click(&p,&app,&mut c,id,40);
    let clicked=p.diagnostic_text_visual(915,32.0,306.0).unwrap();
    assert_eq!(clicked.caret_grapheme,Some(1));
    assert!((clicked.caret_screen_x-first.caret_screen_x).abs()<0.2);
    // A native framebuffer sample proves the white vertical caret was drawn
    // within the ligature's interval, beyond pure logical assertions.
    let sample=p.diagnostic_capture_pixel(clicked.caret_screen_x.floor(),77.0).unwrap();
    assert!(sample.r>=160&&sample.g>=160&&sample.b>=160,
        "native caret pixel at GDEF-proven position must be painted: {sample:?}");
    drop(p);
}
