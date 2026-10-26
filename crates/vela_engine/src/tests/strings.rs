use rstest::rstest;
use vela_common::SourceId;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;

fn run_linked_program(engine: &Engine, source: &str) -> OwnedValue {
    let program = engine
        .compile_source_with_id(SourceId::new(1), source)
        .expect("string test program should compile");
    let linked = engine
        .link_test_program(&program)
        .expect("string test program should link");
    engine
        .into_vm_for_program(&program)
        .run_linked_program(&linked, "main", &[])
        .expect("string test program should run")
}

#[rstest]
#[case::multiline("fn main() { return \"\"\"line1\nline2\"\"\"; }", "line1\nline2")]
#[case::interpolated(
    r#"
fn main() {
    let name = "gold";
    let amount = 7;
    return f"reward {name}: {amount}";
}
"#,
    "reward gold: 7"
)]
#[case::escaped_braces(r#"fn main() { return f"{{status}} {"ok"}"; }"#, "{status} ok")]
fn executes_string_literals(#[case] source: &str, #[case] expected: &str) {
    let engine = Engine::builder().build().expect("engine should build");
    let value = run_linked_program(&engine, source);

    assert_eq!(value, OwnedValue::String(expected.to_owned()));
}
