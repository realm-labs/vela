use super::*;

#[test]
fn syntax_only_script_method_call_lowers_to_method_id_with_default_arg() {
    let source = SourceId::new(1);
    let text = r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus = 4) -> i64 {
        return self.amount + bonus;
    }
}

fn main() {
    return Counter { amount: 3 }.add();
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");

    assert!(
        body_has_no_statement_fallbacks(&payload.body),
        "syntax-only script method call should not retain statement fallbacks"
    );

    let program =
        compile_program_source(source, text).expect("CST-backed script method call should compile");
    let main = program.function("main").expect("main bytecode");
    let method_calls = main
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId { method, args, .. } => {
                Some((method.as_str(), args.as_slice()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(method_calls.len(), 1);
    assert_eq!(method_calls[0].0, "add");
    assert_eq!(method_calls[0].1, &[CallArgument::Missing]);
    assert!(
        main.instructions.iter().all(|instruction| !matches!(
            instruction.kind,
            UnlinkedInstructionKind::CallDynamicMethod { .. }
        )),
        "known script receiver should not lower through dynamic method dispatch"
    );
}

#[test]
fn script_method_call_requires_cst_callee_payload() {
    let source = SourceId::new(1);
    let text = r#"
struct Counter { amount: i64 }

impl Counter {
    fn add(self, bonus = 4) -> i64 {
        return self.amount + bonus;
    }
}

fn main() {
    let counter = Counter { amount: 3 };
    let result = counter.add();
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let facts = cst_payload_compiler_facts(&semantic);
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = call_statement_payloads(&payload.body);
    let call_payload = statements[1]
        .let_initializer_expression_payload()
        .expect("script method call payload");
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("setup binding should compile");
    compiler
        .compile_expr_with_payload(call_payload.fallback(), None)
        .expect("owned fallback call should still compile as a closure-style call");

    assert!(
        compiler.code.instructions.iter().all(|instruction| {
            !matches!(
                instruction.kind,
                UnlinkedInstructionKind::CallMethodId { ref method, .. } if method == "add"
            )
        }),
        "script method lowering must require the CST callee payload"
    );
}
