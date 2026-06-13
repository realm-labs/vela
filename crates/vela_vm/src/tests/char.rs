use super::*;

fn run_char_source(source: &str) -> VmResult<OwnedValue> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program =
        compile_program_source_with_registry(SourceId::new(1), source, registry.compile_view())
            .expect("char source compiles");
    let mut linker = Linker::with_registry(&registry);
    let vm = Vm::new().with_standard_natives();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(&program)
        .expect("char source should link");
    vm.run_linked_program(&linked, "main", &[])
}

#[test]
fn char_methods_match_rust_char_semantics() {
    assert_eq!(
        run_char_source(
            r#"
fn main() {
    if '奖'.to_string() == "奖"
        && ' '.is_whitespace()
        && '7'.is_ascii()
        && '7'.is_ascii_digit()
        && !'奖'.is_ascii()
    {
        return true;
    }
    return false;
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
}
