use super::*;

#[test]
fn semantic_schema_defaults_keep_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Reward {
    amount: i64 = {
        let base = 1;
        base
    }
    label: String
}

fn build() {
    return Reward { label: "xp" };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let type_symbols = semantic.type_symbols();
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let reward = schema_defaults
        .record("Reward")
        .expect("Reward constructor shape");
    let default = reward
        .defaults()
        .next()
        .expect("amount field should carry a default");

    assert_eq!(default.value.source(), source);
    assert_eq!(
        default.value.syntax().syntax().text().to_string(),
        "{\n        let base = 1;\n        base\n    }"
    );

    compile_program_source(source, text).expect("CST-backed schema defaults should compile");
}
