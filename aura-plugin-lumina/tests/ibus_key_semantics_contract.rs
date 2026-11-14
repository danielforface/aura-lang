// LUMINA-IBUS-INPUT-018 test-first source contract.
// A passing source contract alone is NOT live-engine proof: native XTest tests
// must independently exercise the real daemon, engine and GPU editor pipeline.
const HOST:&str=include_str!("../src/lib.rs");
const CLIENT:&str=include_str!("../src/ibus_native.rs");
const HELPER:&str=include_str!("../native/ibus_input_helper.c");
#[test]
fn keyboard_protocol_carries_explicit_modifiers_and_release_not_only_ascii(){
    assert!(CLIENT.contains("struct NativeKeyEvent"),
        "missing typed IBus keyboard frame distinct from Unicode char");
    assert!(CLIENT.contains("process_ibus_key_event"),
        "missing bounded typed IBus keyboard dispatch");
    assert!(HELPER.contains("IBUS_RELEASE_MASK"),
        "missing release-state forwarding to native libibus");
    assert!(HELPER.contains("IBUS_SHIFT_MASK"),
        "missing true modifier-state forwarding to native libibus");
}
#[test]
fn real_raycast_key_path_forwards_named_navigation_to_live_ibus(){
    assert!(HOST.contains("KeyboardKey::KEY_UP") &&
            HOST.contains("KeyboardKey::KEY_DOWN") &&
            HOST.contains("process_ibus_key_event"),
        "existing raylib native keyboard events not wired to IBus");
}
