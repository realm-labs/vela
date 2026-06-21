use super::*;
use crate::compiler::schema_defaults::{SchemaDefaultValue, SchemaFieldDefault};

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

#[test]
fn constant_schema_defaults_compile_without_legacy_fallback() {
    let source = SourceId::new(1);
    let text = r#"
struct Reward {
    amount: i64 = 7
}

fn main() {
    return Reward {};
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
    let value_without_legacy =
        SchemaDefaultValue::new(default.value.source(), default.value.syntax().clone());
    let synthetic_default = SchemaFieldDefault {
        name: default.name.clone(),
        value: value_without_legacy,
        constants: default.constants.clone(),
    };
    with_cst_payload_compiler(text, |compiler, _| {
        let mut fields = Vec::new();
        let explicit_names = std::collections::BTreeSet::new();
        compiler
            .compile_schema_default_fields(
                &mut fields,
                &explicit_names,
                vec![synthetic_default],
                None,
            )
            .expect("constant CST default should compile without legacy fallback");
        assert_eq!(fields.len(), 1);
    });
}

#[test]
fn constant_block_schema_defaults_compile_from_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Reward {
    amount: i64 = {
        let base = 6;
        base + 1
    }
}

fn main() {
    return Reward {};
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
    let value = SchemaDefaultValue::new(default.value.source(), default.value.syntax().clone());
    let synthetic_default = SchemaFieldDefault {
        name: default.name.clone(),
        value,
        constants: default.constants.clone(),
    };
    with_cst_payload_compiler(text, |compiler, _| {
        let mut fields = Vec::new();
        let explicit_names = std::collections::BTreeSet::new();
        compiler
            .compile_schema_default_fields(
                &mut fields,
                &explicit_names,
                vec![synthetic_default],
                None,
            )
            .expect("constant block CST default should compile without legacy fallback");
        assert_eq!(fields.len(), 1);
    });
}

#[test]
fn semantic_tuple_constructor_argument_names_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
enum Damage {
    Magical(amount: i64, element: String)
}

fn build() {
    return Damage::Magical(
        element = {
            let label = "fire";
            label
        },
        amount = {
            let base = 7;
            base
        },
    );
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("build").expect("build function");
    let argument_payloads = payload
        .body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(|expression| expression.call_argument_payloads().unwrap_or_default())
        .collect::<Vec<_>>();
    let names = argument_payloads
        .iter()
        .filter_map(|argument| argument.syntax_name())
        .collect::<Vec<_>>();
    assert_eq!(names, ["element", "amount"]);

    let argument_bodies = argument_payloads
        .into_iter()
        .filter_map(|argument| argument.value_expression_payload().block_body_payload())
        .map(|body| cst_statement_texts(&body))
        .collect::<Vec<_>>();
    assert_eq!(
        argument_bodies,
        vec![
            vec![
                (SyntaxStatementKind::Let, "let label = \"fire\";".to_owned()),
                (SyntaxStatementKind::Expr, "label".to_owned()),
            ],
            vec![
                (SyntaxStatementKind::Let, "let base = 7;".to_owned()),
                (SyntaxStatementKind::Expr, "base".to_owned()),
            ],
        ]
    );

    compile_program_source(source, text)
        .expect("CST-backed named tuple constructor arguments should compile");
}

#[test]
fn tuple_constructor_with_non_path_cst_callee_does_not_use_legacy_constructor() {
    with_cst_payload_compiler(
        r#"
enum Boxed {
    Value(value)
}

fn main() {
    let callable = |value| value;
    let cst_call = ({
        let selected = callable;
        selected
    })(1);
    let legacy_call = Boxed::Value(1);
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[1]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy tuple constructor fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err(
                    "mismatched non-path CST callee must not use the legacy tuple constructor",
                );

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("callable expression")
                ),
                "expected unsupported callable expression, got {error:?}"
            );
        },
    );
}

#[test]
fn semantic_record_constructor_diagnostics_prefer_cst_payload_labels() {
    let source = SourceId::new(1);
    let text = r#"
struct Reward {
    item_id: String
    count: i64
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let type_symbols = semantic.type_symbols();
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let shape = schema_defaults
        .record("Reward")
        .expect("Reward constructor shape");

    let fields = vec![
        record_field("legacy_one", source, 10, 20),
        record_field("legacy_two", source, 21, 30),
        record_field("legacy_three", source, 31, 40),
    ];
    let duplicate_labels = vec![
        Some("item_id".to_owned()),
        Some("item_id".to_owned()),
        Some("count".to_owned()),
    ];
    let diagnostics = crate::compiler::schema_defaults::record_constructor_diagnostics(
        "Reward",
        Some(shape),
        &fields,
        Some(&duplicate_labels),
        Span::new(source, 0, 40),
    );
    assert_eq!(
        diagnostic_codes(&diagnostics),
        ["compiler::duplicate_constructor_field"]
    );
    assert!(diagnostics[0].message.contains("item_id"));
    assert!(!diagnostics[0].message.contains("legacy_one"));

    let unknown_labels = vec![
        Some("item_id".to_owned()),
        Some("count".to_owned()),
        Some("bonus".to_owned()),
    ];
    let diagnostics = crate::compiler::schema_defaults::record_constructor_diagnostics(
        "Reward",
        Some(shape),
        &fields,
        Some(&unknown_labels),
        Span::new(source, 0, 40),
    );
    assert_eq!(
        diagnostic_codes(&diagnostics),
        ["compiler::unknown_constructor_field"]
    );
    assert!(diagnostics[0].message.contains("bonus"));
    assert!(!diagnostics[0].message.contains("legacy_three"));
}

fn record_field(
    name: &str,
    source: SourceId,
    start: u32,
    end: u32,
) -> vela_syntax::ast::RecordField {
    vela_syntax::ast::RecordField {
        name: name.to_owned(),
        span: Span::new(source, start, end),
        value: None,
    }
}

fn diagnostic_codes(diagnostics: &[vela_common::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.code.as_deref())
        .collect()
}
