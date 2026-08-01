use super::*;

#[test]
fn compiler_resolves_method_call_receiver_from_hir() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

impl Counter {
    fn add(self, amount: i64) {
        return self.value + amount;
    }
}

fn sample(counter: Counter) {
    return counter.add(2);
}
"#,
    )
    .expect("script method call should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::CallMethodId { .. }
        )),
        "script method call should lower to a resolved method call"
    );
}

#[test]
fn compiler_resolves_record_field_read_receiver_from_hir() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

fn sample(counter: Counter) {
    return counter.value;
}
"#,
    )
    .expect("record field read should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot { .. }
        )),
        "typed record field read should lower to a resolved slot"
    );
}
