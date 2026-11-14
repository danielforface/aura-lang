// Phase 25 semantic RED: a raw IBus D request is not an edit.
// Only a real-engine D followed by its C in one native key transaction,
// owned by the focused original authoritative snapshot, may replace text.
const EDITOR:&str=include_str!("../src/text_input.rs");
const NATIVE:&str=include_str!("../src/ibus_native.rs");
const HOST:&str=include_str!("../src/lib.rs");
const LIVE:&str=include_str!("ibus_hangul_delete_e2e.rs");
#[test]fn exact_epoch_atomic_delete_plus_commit_one_undo_and_one_callback(){
    assert!(EDITOR.contains("fn replace_surrounding_if_snapshot"));
    assert!(EDITOR.contains("rebase_generation"));
    assert!(NATIVE.contains("struct NativeReplace"));
    assert!(NATIVE.contains("fn take_native_replacements"));
    assert!(HOST.contains("native_replacements"));
}
#[test]fn registered_hangul_real_d_then_c_has_no_duplicate_syllables(){
    assert!(LIVE.contains("real_hangul_none_preedit_atomic_replace_and_undo"));
    assert!(LIVE.contains("LUMINA_IBUS_P25_ATOMIC_REPLACE"));
    assert!(NATIVE.contains("sync_authority_generation"));
}
