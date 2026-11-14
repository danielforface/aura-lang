// Phase 24 RED: native third-party registered ibus-hangul engine must
// genuinely emit a D request in a new Linux XTest suite (no fake D frames).
const IBUS:&str=include_str!("../src/ibus_native.rs");
const LUMINA:&str=include_str!("../src/lib.rs");
const LIVE:&str=include_str!("ibus_hangul_delete_e2e.rs");
const WORKFLOW:&str=include_str!("../../.github/workflows/lumina-viewport-headless.yml");
#[test]fn native_delete_signal_has_focus_owned_evidence_without_editor_mutation(){
    assert!(IBUS.contains("surrounding_last_delete"));
    assert!(LUMINA.contains("diagnostic_native_ibus_deletion"));
}
#[test]fn genuine_hangul_none_mode_registered_daemon_delete_proof_is_required(){
    assert!(LIVE.contains("real_hangul_none_preedit_engine_emits_delete_surrounding"));
    assert!(WORKFLOW.contains("preedit-mode"));
    assert!(WORKFLOW.contains("ibus_hangul_delete_e2e"));
}
