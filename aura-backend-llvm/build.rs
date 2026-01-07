fn main() {
    // Intentionally empty: the current LLVM backend emits textual LLVM IR and does not
    // link against system LLVM libraries.
    println!("cargo:rerun-if-changed=build.rs");
}
