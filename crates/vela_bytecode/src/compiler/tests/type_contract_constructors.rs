use super::*;

#[test]
fn compiler_contextualizes_typed_record_constructor_literals_without_guard() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: u8,
}
fn main() {
    return Reward { amount: 12 };
}
"#,
    )
    .expect("typed record constructor literal should be contextualized");
    let main = program.function("main").expect("main function");

    assert!(
        !main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
    assert!(
        main.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
    assert!(
        !main
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn compiler_emits_field_guard_for_dynamic_typed_record_constructor() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64,
}
fn main(value) {
    return Reward { amount: value };
}
"#,
    )
    .expect("dynamic record constructor field should compile with runtime guard");
    let main = program.function("main").expect("main function");
    let guard_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::GuardType { .. })
        })
        .expect("dynamic record constructor field should emit GuardType");
    let make_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::MakeRecord { .. })
        })
        .expect("record constructor should emit MakeRecord");

    assert!(guard_index < make_index);
    let UnlinkedInstructionKind::GuardType {
        src: guard_src,
        guard,
    } = &main.instructions[guard_index].kind
    else {
        panic!("expected GuardType");
    };
    let UnlinkedInstructionKind::MakeRecord { fields, .. } = &main.instructions[make_index].kind
    else {
        panic!("expected MakeRecord");
    };

    assert_eq!(fields, &vec![("amount".to_owned(), *guard_src)]);
    assert_eq!(guard.context.location, crate::GuardLocation::Field);
    assert_eq!(guard.context.debug_name, "amount");
    assert!(matches!(
        guard.plan,
        crate::UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64)
    ));
}

#[test]
fn compiler_rejects_static_typed_record_constructor_mismatches() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64,
}
fn main() {
    return Reward { amount: "x" };
}
"#,
    )
    .expect_err("static record constructor mismatch should fail before bytecode emission");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::type_contract_mismatch"]
    );
}

#[test]
fn compiler_contextualizes_typed_enum_payload_literals_without_guard() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical(amount: u8),
    Magical { amount: u16 },
}
fn tuple_payload() {
    return Damage::Physical(12);
}
fn record_payload() {
    return Damage::Magical { amount: 300 };
}
"#,
    )
    .expect("typed enum payload literals should be contextualized");
    let tuple_payload = program
        .function("tuple_payload")
        .expect("tuple payload function");
    let record_payload = program
        .function("record_payload")
        .expect("record payload function");

    for function in [tuple_payload, record_payload] {
        assert!(!function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        )));
    }
    assert!(
        tuple_payload
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
    assert!(
        record_payload
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U16(300)))
    );
}

#[test]
fn compiler_emits_field_guard_for_dynamic_typed_enum_payload() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical(amount: i64),
}
fn main(value) {
    return Damage::Physical(value);
}
"#,
    )
    .expect("dynamic enum payload should compile with runtime guard");
    let main = program.function("main").expect("main function");
    let guard = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::GuardType { guard, .. } => Some(guard),
            _ => None,
        })
        .expect("dynamic enum payload should emit GuardType");

    assert_eq!(guard.context.location, crate::GuardLocation::Field);
    assert_eq!(guard.context.debug_name, "0");
    assert!(matches!(
        guard.plan,
        crate::UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64)
    ));
}

#[test]
fn compiler_rejects_static_typed_enum_payload_mismatches() {
    for source in [
        r#"
enum Damage {
    Physical(amount: i64),
}
fn main() {
    return Damage::Physical("x");
}
"#,
        r#"
enum Damage {
    Magical { amount: i64 },
}
fn main() {
    return Damage::Magical { amount: "x" };
}
"#,
    ] {
        let error = compile_program_source(SourceId::new(1), source)
            .expect_err("static enum payload mismatch should fail before bytecode emission");
        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::type_contract_mismatch"]
        );
    }
}
