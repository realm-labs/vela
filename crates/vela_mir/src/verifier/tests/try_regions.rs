use super::*;

use vela_def::{FieldId, VariantId};

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileTryFamily, CompileTryLayoutTarget,
    CompileTryTarget, CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor,
    MirFieldTarget, MirTryContinue,
};

const OPTION: TypeId = TypeId::new(7_800);
const SOME: VariantId = VariantId::new(7_801);
const NONE: VariantId = VariantId::new(7_802);
const PAYLOAD: FieldId = FieldId::new(7_803);
const LAYOUT: CompileTryLayoutTarget = CompileTryLayoutTarget {
    family: CompileTryFamily::Option,
    type_id: OPTION,
    continue_variant: SOME,
    break_variant: NONE,
    continue_payload: PAYLOAD,
};
const TARGET: CompileTryTarget = CompileTryTarget::Expected(LAYOUT);

fn try_table() -> MirTargetTable {
    let mut table = target_table();
    assert!(table.insert_type(CompileTypeDescriptor {
        id: OPTION,
        canonical_name: "std::Option".to_owned(),
        runtime_name: "Option".to_owned(),
        class: CompileTypeClass::Standard,
        shape: None,
        fields: Vec::new(),
        variants: vec![SOME, NONE],
    }));
    assert!(table.insert_variant(CompileVariantDescriptor {
        id: SOME,
        owner: OPTION,
        name: "Some".to_owned(),
        fields: vec![PAYLOAD],
        declaration_order: 0,
    }));
    assert!(table.insert_variant(CompileVariantDescriptor {
        id: NONE,
        owner: OPTION,
        name: "None".to_owned(),
        fields: Vec::new(),
        declaration_order: 1,
    }));
    assert!(table.insert_field(CompileFieldDescriptor {
        id: PAYLOAD,
        owner: OPTION,
        variant: Some(SOME),
        name: "0".to_owned(),
        contract: None,
        declaration_order: 0,
        access: CompileFieldAccess::script(),
        host_runtime: None,
    }));
    table
}

fn canonical_try(extra_continue_statement: bool) -> crate::MirProgram {
    let mut function = function();
    let root = function.entry_block();
    let join = function.add_block();
    let propagate = function.add_block();
    let invalid = function.add_block();
    let continuation = function.add_block();
    let value = function.add_synthetic_local(MirValueType::Enum(OPTION), origin());
    let result = function.add_synthetic_local(MirValueType::Dynamic, origin());
    let safepoint = function.add_safepoint(MirSafepoint::new(origin()));
    function
        .append_statement(
            root,
            MirStatement::new(
                origin(),
                Some(MirPlace::local(value)),
                MirStatementKind::Allocate(MirAggregate::Enum {
                    type_id: OPTION,
                    variant: SOME,
                    fields: vec![(PAYLOAD, scalar(7))],
                }),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )
        .expect("try operand");
    function
        .set_terminator(
            root,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::TrySwitch {
                    value: MirOperand::Local(value),
                    target: TARGET,
                    result,
                    continuations: vec![MirTryContinue {
                        layout: LAYOUT,
                        block: continuation,
                    }],
                    propagate,
                    invalid,
                    join,
                },
                MirEffect::PURE,
                None,
            ),
        )
        .expect("try switch");
    function
        .append_statement(
            continuation,
            MirStatement::new(
                origin(),
                Some(MirPlace::local(result)),
                MirStatementKind::ReadField {
                    receiver: MirOperand::Local(value),
                    target: MirFieldTarget::VariantSlot {
                        type_id: OPTION,
                        variant: SOME,
                        field: PAYLOAD,
                    },
                },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("payload read");
    if extra_continue_statement {
        let temp = function.add_temp(MirValueType::Dynamic, origin());
        function
            .append_statement(
                continuation,
                MirStatement::assign(
                    origin(),
                    MirPlace::temp(temp),
                    MirRvalue::Use(MirOperand::Local(result)),
                ),
            )
            .expect("noncanonical extra statement");
    }
    function
        .set_terminator(
            continuation,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Jump(join),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("continue join");
    function
        .set_terminator(
            propagate,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Local(value))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("propagate");
    function
        .set_terminator(
            invalid,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::TryTypeMismatch { target: TARGET },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("invalid");
    function
        .set_terminator(
            join,
            MirTerminator::new(
                origin(),
                MirTerminatorKind::Return(Some(MirOperand::Local(result))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("join");
    let mut program = crate::MirProgram::new(try_table());
    program.add_function(function).expect("try function");
    program
}

#[test]
fn mir_verifier_accepts_only_the_canonical_try_region_shape() {
    verify_mir(&canonical_try(false)).expect("canonical try region verifies");
    assert!(matches!(
        verify_error(&canonical_try(true)).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));
}

#[test]
fn mir_verifier_rejects_try_type_mismatch_outside_a_region() {
    let mut function = function();
    function
        .set_terminator(
            function.entry_block(),
            MirTerminator::new(
                origin(),
                MirTerminatorKind::TryTypeMismatch { target: TARGET },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("standalone mismatch");
    let mut program = crate::MirProgram::new(try_table());
    program.add_function(function).expect("try function");
    assert!(matches!(
        verify_error(&program).into_kind(),
        MirVerifyErrorKind::InvalidTerminatorContract(_)
    ));
}
