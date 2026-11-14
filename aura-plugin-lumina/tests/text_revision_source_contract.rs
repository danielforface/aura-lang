// RED-to-GREEN source contracts; separate real XTest proves runtime behavior.
#[test]
fn explicit_revision_opt_in_guards_authority_and_old_echoes() {
 let lib=include_str!("../src/lib.rs");
 let model=include_str!("../src/text_revision.rs");
 assert!(lib.contains("mod text_revision;"),"RED: no versioned revision protocol");
 assert!(lib.contains("value_revision"),"RED: native host input lacks revision prop");
 assert!(lib.contains("revision.observe("),"RED: no authoritative vs echo reconciliation");
 assert!(model.contains("struct RevisionTracker"),"RED: no independent state machine");
}
#[test]
fn local_edits_are_recorded_for_asynchronous_echo_classification() {
 let lib=include_str!("../src/lib.rs");
 assert!(lib.contains("revision.record_local_edit("),"RED: editor does not track emitted values");
}
