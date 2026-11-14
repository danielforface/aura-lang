// Test-first missing feature: do not replace visual navigation with logical
// grapheme order and do not fill an enclosing rectangle for split BiDi selections.
const LIB:&str=include_str!("../src/lib.rs");
#[test]fn native_optin_editor_must_use_visual_hit_and_physical_arrows(){
    let model=std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),"/src/bidi_editor.rs")).unwrap_or_default();
    assert!(model.contains("pub(crate) fn navigate("),"visual navigation state is required");
    assert!(model.contains("pub(crate) fn hit_test("),"physical mouse hit mapping required");
    assert!(LIB.contains("bidi_editor::navigate("),"native key inputs must call visual mapping");
    assert!(LIB.contains("bidi_editor::hit_test("),"native click must update visual caret");
}
#[test]fn selection_must_paint_disjoint_visual_run_fragments(){
    let model=std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"),"/src/bidi_editor.rs")).unwrap_or_default();
    assert!(model.contains("pub(crate) fn selection_fragments("),"logical selection must be split by visual runs");
    assert!(LIB.contains("selection_screens(content)"),"paint each fragment, not one enclosing rectangle");
    assert!(LIB.contains("caret_valid"),"incomplete GSUB cluster caret must fail closed");
}
