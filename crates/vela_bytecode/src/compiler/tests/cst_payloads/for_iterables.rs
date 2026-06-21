use super::*;

#[test]
fn semantic_function_for_iterable_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn loop_values() {
    let total = 0;
    for value in {
        let start = 0;
        start
    }..{
        let end = 3;
        end
    } {
        total += value;
    }
    for value in {
        let values = [1, 2];
        values
    } {
        total += value;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("loop_values")
        .expect("loop_values function");

    let iterable_payloads = payload
        .body
        .statement_payloads()
        .into_iter()
        .filter_map(|statement| statement.for_iterable_expression_payload())
        .collect::<Vec<_>>();
    assert_eq!(iterable_payloads.len(), 2);
    assert_eq!(
        iterable_payloads[0].kind(),
        Some(SyntaxExpressionKind::Binary)
    );
    let (range_start, range_end) = iterable_payloads[0]
        .binary_operand_payloads()
        .expect("range iterable should expose operand payloads");
    assert_eq!(range_start.kind(), Some(SyntaxExpressionKind::Block));
    assert_eq!(range_end.kind(), Some(SyntaxExpressionKind::Block));
    assert_eq!(
        iterable_payloads[1].kind(),
        Some(SyntaxExpressionKind::Block)
    );

    let program = compile_program_source(source, text)
        .expect("CST-backed for iterable values should compile");
    let function = program
        .function("loop_values")
        .expect("loop_values bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64RangeNext { .. }
    )));
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::IterInit { .. }
        ))
    );
}
