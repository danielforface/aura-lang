// LUMINA-IBUS-CANDIDATES-019 genuine missing-feature RED.
// Source contracts do NOT substitute for real native IBus engine proof.
const HELPER:&str=include_str!("../native/ibus_input_helper.c");
const CLIENT:&str=include_str!("../src/ibus_native.rs");
const HOST:&str=include_str!("../src/lib.rs");
const CI:&str=include_str!("../../.github/workflows/lumina-viewport-headless.yml");
#[test]fn real_ibus_lookup_signals_reach_a_bounded_typed_candidate_model(){
    assert!(HELPER.contains("update-lookup-table"));
    assert!(HELPER.contains("hide-lookup-table"));
    assert!(HELPER.contains("ibus_lookup_table_get_candidate"));
    assert!(CLIENT.contains("struct IbusCandidatePage"));
    assert!(CLIENT.contains("parse_lookup_frame"));
}
#[test]fn candidate_lifecycle_is_integrated_with_lumina_and_real_engine_ci(){
    assert!(HOST.contains("diagnostic_native_ibus_candidates"));
    assert!(CI.contains("ibus-libpinyin"));
    assert!(CI.contains("ibus_candidate_e2e"));
}
