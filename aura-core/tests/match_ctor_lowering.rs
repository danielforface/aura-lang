use aura_ir::InstKind;

#[test]
fn lowers_enum_ctor_and_match_patterns_via_tensor_encoding() {
    let src = r#"
import aura::io

type Opt = enum { None, Some(v: u32) }

cell main() ->:
  val mut out: u32 = 0
  val x: Opt = Opt::Some(7)
  match x:
    Opt::Some(n):
      out = n
    Opt::None:
      out = 1
    _:
      out = 2
  io::println("ok")
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let module = aura_core::lower_program(&program).expect("lower");

    let f = module.functions.get("main").expect("main");

    let mut saw_tensor_new = false;
    let mut saw_tensor_set = false;
    let mut saw_tensor_get = false;

    for bb in &f.blocks {
        for inst in &bb.insts {
            if let InstKind::Call { callee, .. } = &inst.kind {
                match callee.as_str() {
                    "tensor.new" => saw_tensor_new = true,
                    "tensor.set" => saw_tensor_set = true,
                    "tensor.get" => saw_tensor_get = true,
                    _ => {}
                }
            }
        }
    }

    assert!(saw_tensor_new, "expected enum ctor lowering to use tensor.new");
    assert!(saw_tensor_set, "expected enum ctor lowering to use tensor.set");
    assert!(saw_tensor_get, "expected enum match lowering to use tensor.get");
}
