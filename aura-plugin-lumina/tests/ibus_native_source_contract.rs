// LUMINA-IBUS-NATIVE-017 test-first RED, no compile-construction failure.
// Verify actual Linux GI/IBus server adapter source AND Rust runtime bridge
// are present; synthetic CompositionEvent injection is not OS qualification.
const HOST:&str=include_str!("../src/lib.rs");
#[test]fn actual_ibus_native_backend_is_present(){
    assert!(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"),"/native/ibus_bridge.py")).is_file());
    assert!(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"),"/src/native_ibus.rs")).is_file());
    assert!(HOST.contains("mod native_ibus;"));
    assert!(HOST.contains("native_ime"));
}
#[test]fn genuine_ibus_daemon_origin_signals_are_gated_in_native_ci(){
    let ci=include_str!("../../.github/workflows/lumina-viewport-headless.yml");
    assert!(ci.contains("ibus-hangul"));
    assert!(ci.contains("ibus-daemon"));
    assert!(ci.contains("ibus_bridge.py"));
}
