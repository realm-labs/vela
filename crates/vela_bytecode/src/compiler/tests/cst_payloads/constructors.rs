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
