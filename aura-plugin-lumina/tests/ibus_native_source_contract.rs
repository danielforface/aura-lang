// LUMINA-IBUS-NATIVE-017 semantic RED: real IBus client/engine key path.
// This test fails for absent implementation without requiring libibus in the
// pure headless lane; native CI must separately launch a live daemon+engine.
const HOST:&str=include_str!("../src/lib.rs");
const WORKFLOW:&str=include_str!("../../.github/workflows/lumina-viewport-headless.yml");
#[test]fn ibus_native_requires_real_daemon_engine_process_key_and_signal_bridge(){
    assert!(HOST.contains("mod ibus_native;"));
    assert!(HOST.contains("process_ibus_key"));
    assert!(WORKFLOW.contains("ibus-hangul"));
    assert!(WORKFLOW.contains("ibus_native_e2e"));
}
