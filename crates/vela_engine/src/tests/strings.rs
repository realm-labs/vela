use vela_common::SourceId;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;

fn run_linked_program(engine: &Engine, source: &str) -> OwnedValue {
    let program = engine
        .compile_source(SourceId::new(1), source)
        .expect("string test program should compile");
    let linked = engine
        .link_program(&program)
        .expect("string test program should link");
    engine
        .into_vm_for_program(&program)
        .run_linked_program(&linked, "main", &[])
        .expect("string test program should run")
}

#[test]
fn executes_multiline_string_literals() {
    let value = run_linked_program(
        &Engine::builder().build().expect("engine should build"),
        "fn main() { return \"\"\"line1\nline2\"\"\"; }",
    );

    assert_eq!(value, OwnedValue::String("line1\nline2".to_owned()));
}

#[test]
fn executes_interpolated_strings() {
    let value = run_linked_program(
        &Engine::builder().build().expect("engine should build"),
        r#"
fn main() {
    let name = "gold";
    let amount = 7;
    return f"reward {name}: {amount}";
}
"#,
    );

    assert_eq!(value, OwnedValue::String("reward gold: 7".to_owned()));
}

#[test]
fn executes_interpolated_strings_with_escaped_braces() {
    let value = run_linked_program(
        &Engine::builder().build().expect("engine should build"),
        r#"fn main() { return f"{{status}} {"ok"}"; }"#,
    );

    assert_eq!(value, OwnedValue::String("{status} ok".to_owned()));
}
