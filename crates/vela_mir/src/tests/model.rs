use vela_common::{HostTypeId, PrimitiveTag, ScalarValue, SourceId, Span};
use vela_def::{FunctionId, GlobalId, MethodId, TypeId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId, HirNodeId};

use crate::*;

fn origin(body: HirBodyId) -> MirSourceOrigin {
    MirSourceOrigin::body(body, Span::new(SourceId::new(7), 0, 5))
}

fn test_function(body: HirBodyId, owner: MirFunctionOwner, origin: MirSourceOrigin) -> MirFunction {
    MirFunction::new(
        body,
        owner,
        format!("test::body_{}", body.get()),
        None,
        origin,
    )
}

#[test]
fn mir_model_enforces_single_assignment_temps_and_mutable_locals() {
    let body = HirBodyId::new(1);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(10)),
        origin,
    );
    let local = function.add_script_local(
        HirLocalId::new(3),
        MirValueType::Primitive(PrimitiveTag::I64),
        origin,
    );
    let temp = function.add_temp(MirValueType::Primitive(PrimitiveTag::I64), origin);
    let entry = function.entry_block();

    let first = function
        .append_statement(
            entry,
            MirStatement::assign(
                origin,
                MirPlace::temp(temp),
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Scalar(
                    ScalarValue::I64(4),
                ))),
            ),
        )
        .expect("first temp definition should be accepted");
    assert_eq!(
        function.temp(temp).and_then(MirTemp::definition),
        Some(first)
    );
    assert_eq!(
        function.append_statement(
            entry,
            MirStatement::assign(
                origin,
                MirPlace::temp(temp),
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Scalar(
                    ScalarValue::I64(5)
                ))),
            ),
        ),
        Err(MirBuildError::TempAlreadyDefined { temp, origin })
    );

    function
        .append_statement(
            entry,
            MirStatement::assign(
                origin,
                MirPlace::local(local),
                MirRvalue::Use(MirOperand::Temp(temp)),
            ),
        )
        .expect("local assignment should be accepted");
    function
        .append_statement(
            entry,
            MirStatement::assign(
                origin,
                MirPlace::local(local),
                MirRvalue::Use(MirOperand::Immediate(MirImmediate::Scalar(
                    ScalarValue::I64(6),
                ))),
            ),
        )
        .expect("mutable local should accept a later assignment");
}

#[test]
fn mir_model_requires_safepoints_for_calls_even_with_incomplete_effect_metadata() {
    let body = HirBodyId::new(2);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(20)),
        origin,
    );
    let entry = function.entry_block();
    let destination = function.add_temp(MirValueType::Dynamic, origin);
    let call = MirStatement::new(
        origin,
        Some(MirPlace::temp(destination)),
        MirStatementKind::Call(MirCall::NativeFunction {
            function: FunctionId::new(21),
            debug_name: "native::test".to_owned(),
            signature: CompileSignature {
                parameters: Vec::new(),
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            },
            arguments: Vec::new(),
        }),
        MirEffect::may_trap(),
        None,
    );
    assert_eq!(
        function.append_statement(entry, call.clone()),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::external_call(),
            actual: MirEffect::may_trap(),
        })
    );

    let call = MirStatement {
        effect: MirEffect::external_call(),
        ..call
    };
    assert_eq!(
        function.append_statement(entry, call.clone()),
        Err(MirBuildError::MissingSafepoint { origin })
    );

    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    let call = MirStatement {
        safepoint: Some(safepoint),
        ..call
    };
    function
        .append_statement(entry, call)
        .expect("call with a valid safepoint should be accepted");
}

#[test]
fn mir_model_rejects_effect_and_destination_contradictions() {
    let body = HirBodyId::new(21);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(210)),
        origin,
    );
    let entry = function.entry_block();
    let destination = function.add_temp(MirValueType::Dynamic, origin);
    let write = MirStatement::new(
        origin,
        Some(MirPlace::temp(destination)),
        MirStatementKind::WriteField {
            receiver: MirOperand::Immediate(MirImmediate::Unit),
            target: MirFieldTarget::Dynamic {
                name: "value".to_owned(),
            },
            value: MirOperand::Immediate(MirImmediate::Unit),
        },
        MirEffect::may_trap(),
        None,
    );
    assert_eq!(
        function.append_statement(entry, write),
        Err(MirBuildError::UnexpectedStatementDestination { origin })
    );

    let write = MirStatement::new(
        origin,
        None,
        MirStatementKind::WriteField {
            receiver: MirOperand::Immediate(MirImmediate::Unit),
            target: MirFieldTarget::Dynamic {
                name: "value".to_owned(),
            },
            value: MirOperand::Immediate(MirImmediate::Unit),
        },
        MirEffect::PURE,
        None,
    );
    assert_eq!(
        function.append_statement(entry, write),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::may_trap(),
            actual: MirEffect::PURE,
        })
    );

    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    let call = MirStatement::new(
        origin,
        Some(MirPlace::temp(destination)),
        MirStatementKind::Call(MirCall::ScriptFunction {
            function: FunctionId::new(212),
            debug_name: "script::test".to_owned(),
            signature: CompileSignature {
                parameters: vec![CompileParameter {
                    name: "value".to_owned(),
                    contract: None,
                    default: CompileParameterDefault::Required,
                    origin: None,
                }],
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            },
            arguments: vec![MirScriptArgument::missing(0)],
        }),
        MirEffect::script_call(),
        Some(safepoint),
    );
    assert_eq!(
        function.append_statement(entry, call),
        Err(MirBuildError::InvalidCallArgumentPlacement { origin })
    );
}

#[test]
fn mir_model_exposes_allocating_host_reflection_and_dynamic_boundaries() {
    let body = HirBodyId::new(28);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(280)),
        origin,
    );
    let entry = function.entry_block();
    let host_type = HostTypeTarget {
        semantic: TypeId::new(281),
        runtime: HostTypeId::new(282),
    };
    let root = function.add_synthetic_local(MirValueType::Host(host_type), origin);
    let host_result = function.add_temp(MirValueType::Dynamic, origin);
    let host_read = MirStatement::new(
        origin,
        Some(MirPlace::temp(host_result)),
        MirStatementKind::Host(MirHostOperation::Read {
            root: MirOperand::Local(root),
            path: MirHostPath {
                root_type: host_type,
                segments: vec![MirHostPathSegment::Field(HostFieldTarget {
                    owner: host_type,
                    semantic: vela_def::FieldId::new(283),
                    runtime: vela_def::FieldId::new(284),
                    readable: true,
                    writable: false,
                })],
            },
        }),
        MirEffect {
            may_trap: true,
            host_read: true,
            ..MirEffect::PURE
        },
        None,
    );
    assert_eq!(
        function.append_statement(entry, host_read.clone()),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::host_read(),
            actual: host_read.effect,
        })
    );
    let host_read = MirStatement {
        effect: MirEffect::host_read(),
        ..host_read
    };
    assert_eq!(
        function.append_statement(entry, host_read.clone()),
        Err(MirBuildError::MissingSafepoint { origin })
    );
    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    function
        .append_statement(
            entry,
            MirStatement {
                safepoint: Some(safepoint),
                ..host_read
            },
        )
        .expect("host reads can materialize host strings/bytes in the script heap");

    let static_member = function.add_temp(MirValueType::Primitive(PrimitiveTag::String), origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(static_member)),
                MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(
                    "level".to_owned(),
                )),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )
        .expect("reflection string members must materialize as explicit heap operands");
    let reflection_write = MirStatement::new(
        origin,
        None,
        MirStatementKind::Reflect(MirReflectionOperation::Write {
            function: FunctionId::new(286),
            target: MirOperand::Local(root),
            member: MirOperand::Temp(static_member),
            value: MirOperand::Immediate(MirImmediate::Unit),
        }),
        MirEffect::reflection_write(),
        Some(safepoint),
    );
    assert!(!reflection_write.effect.reflection_read);
    assert_eq!(
        function.append_statement(entry, reflection_write.clone()),
        Err(MirBuildError::MissingStatementDestination { origin })
    );
    let reflected = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement {
                destination: Some(MirPlace::temp(reflected)),
                ..reflection_write
            },
        )
        .expect("reflection writes retain their observable unit or rebuilt-value result");

    let member =
        function.add_synthetic_local(MirValueType::Primitive(PrimitiveTag::String), origin);
    let dynamic_read_result = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(dynamic_read_result)),
                MirStatementKind::Reflect(MirReflectionOperation::Read {
                    function: FunctionId::new(287),
                    target: MirOperand::Local(root),
                    member: MirOperand::Local(member),
                }),
                MirEffect::reflection_read(),
                Some(safepoint),
            ),
        )
        .expect("reflection reads preserve an evaluated dynamic member operand");

    let callable_result = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(callable_result)),
                MirStatementKind::Reflect(MirReflectionOperation::Call {
                    function: FunctionId::new(288),
                    target: MirOperand::Local(root),
                    tail: vec![MirOperand::Immediate(MirImmediate::Unit)],
                }),
                MirEffect::reflection_call(),
                Some(safepoint),
            ),
        )
        .expect("reflection calls preserve callable-target tail operands");

    let method_result = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(method_result)),
                MirStatementKind::Reflect(MirReflectionOperation::Call {
                    function: FunctionId::new(288),
                    target: MirOperand::Local(root),
                    tail: vec![
                        MirOperand::Local(member),
                        MirOperand::Immediate(MirImmediate::Unit),
                    ],
                }),
                MirEffect::reflection_call(),
                Some(safepoint),
            ),
        )
        .expect("reflection calls preserve dynamic method-name and argument order");

    let dynamic_result = function.add_temp(MirValueType::Dynamic, origin);
    let dynamic = MirStatement::new(
        origin,
        Some(MirPlace::temp(dynamic_result)),
        MirStatementKind::DynamicBinary {
            operation: MirDynamicBinaryOp::Equal,
            left: MirOperand::Immediate(MirImmediate::Unit),
            right: MirOperand::Immediate(MirImmediate::Unit),
        },
        MirEffect::may_trap(),
        Some(safepoint),
    );
    assert_eq!(
        function.append_statement(entry, dynamic),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::dynamic_call(),
            actual: MirEffect::may_trap(),
        })
    );

    let global_result = function.add_temp(MirValueType::Dynamic, origin);
    let incomplete_global = MirEffect {
        global_read: true,
        ..MirEffect::PURE
    };
    let global = MirStatement::new(
        origin,
        Some(MirPlace::temp(global_result)),
        MirStatementKind::Global(MirGlobalOperation::Read {
            global: GlobalId::new(285),
        }),
        incomplete_global,
        None,
    );
    assert_eq!(
        function.append_statement(entry, global),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::global_read(),
            actual: incomplete_global,
        })
    );
}

#[test]
fn mir_model_keeps_contextual_literals_static_keys_and_traps_explicit() {
    let body = HirBodyId::new(29);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(290)),
        origin,
    );
    let entry = function.entry_block();
    let value = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let contextual_result = function.add_temp(MirValueType::Dynamic, origin);
    let contextual = MirStatement::new(
        origin,
        Some(MirPlace::temp(contextual_result)),
        MirStatementKind::ContextualNumericBinary {
            operation: MirContextualBinaryOp::Add,
            value: MirOperand::Local(value),
            literal: MirContextualNumericLiteral::Integer(vela_hir::body::HirIntegerLiteral {
                text: "0x12c".to_owned(),
                radix: vela_hir::body::HirIntRadix::Hex,
                suffix: None,
            }),
            literal_side: MirLiteralSide::Right,
        },
        MirEffect::PURE,
        None,
    );
    assert_eq!(
        function.append_statement(entry, contextual),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::may_trap(),
            actual: MirEffect::PURE,
        })
    );

    let tuple_result = function.add_temp(MirValueType::Dynamic, origin);
    let tuple_field = MirStatement::new(
        origin,
        Some(MirPlace::temp(tuple_result)),
        MirStatementKind::TupleField {
            tuple: MirOperand::Local(value),
            index: 2,
        },
        MirEffect::PURE,
        None,
    );
    assert_eq!(
        function.append_statement(entry, tuple_field),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::may_trap(),
            actual: MirEffect::PURE,
        })
    );

    let truthy = function.add_temp(MirValueType::Primitive(PrimitiveTag::Bool), origin);
    function
        .append_statement(
            entry,
            MirStatement::assign(
                origin,
                MirPlace::temp(truthy),
                MirRvalue::Truthy {
                    value: MirOperand::Local(value),
                },
            ),
        )
        .expect("truthiness is a non-allocating tag test");
    let identity = function.add_temp(MirValueType::Primitive(PrimitiveTag::Bool), origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(identity)),
                MirStatementKind::IdentityCompare {
                    operation: MirIdentityOp::Equal,
                    left: MirOperand::Local(value),
                    right: MirOperand::Local(value),
                },
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("identity comparison is explicit and cannot call script traits");
    let indexed = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(indexed)),
                MirStatementKind::Index(MirIndexOperation::Read {
                    receiver: MirOperand::Local(value),
                    index: MirIndexKey::ConstantString("level".to_owned()),
                }),
                MirEffect::may_trap(),
                None,
            ),
        )
        .expect("constant string indexing should not materialize a heap string");
    let map = function.add_temp(MirValueType::Dynamic, origin);
    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(map)),
                MirStatementKind::Allocate(MirAggregate::Map(vec![(
                    "score".to_owned(),
                    MirOperand::Local(value),
                )])),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )
        .expect("map literal keys should retain their HIR-owned static spelling");
    let set = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(set)),
                MirStatementKind::Allocate(MirAggregate::SetFromArray {
                    source: MirOperand::Local(value),
                }),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )
        .expect("set construction preserves its evaluated array source and allocation boundary");
}

#[test]
fn mir_model_snapshot_owns_method_signatures_and_guard_contracts() {
    let method = MethodId::new(215);
    let fact_origin = origin(HirBodyId::new(21));
    let guard_key = CompileGuardKey::Expression(HirExprId::new(216));
    let guard = CompileGuardTarget {
        contract: MirTypeContract::Array(Some(Box::new(MirTypeContract::Primitive(
            PrimitiveTag::I64,
        )))),
        debug_name: "values".to_owned(),
    };
    let signature = CompileSignature {
        parameters: vec![CompileParameter {
            name: "value".to_owned(),
            contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            default: CompileParameterDefault::RuntimeProvided,
            origin: None,
        }],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: Some(MirTypeContract::Primitive(PrimitiveTag::Bool)),
        effect: MirEffect::host_read(),
    };
    let descriptor = CompileMethodDescriptor {
        id: method,
        owner: TypeId::new(217),
        member_name: "push".to_owned(),
        debug_name: "Array::push".to_owned(),
        class: CompileMethodClass::Value,
        signature: signature.clone(),
    };
    let type_id = TypeId::new(218);
    let variant_id = vela_def::VariantId::new(219);
    let field_id = vela_def::FieldId::new(220);
    let global_id = GlobalId::new(221);
    let global_declaration = HirDeclId::new(222);
    let mut snapshot = CompileTargetSnapshot::builder();
    snapshot
        .insert_method_descriptor(descriptor.clone(), fact_origin)
        .expect("method descriptor should be unique");
    let mut duplicate = descriptor.clone();
    duplicate.debug_name = "corrupt".to_owned();
    assert!(matches!(
        snapshot.insert_method_descriptor(duplicate, fact_origin),
        Err(MirBuildError::InconsistentInput { .. })
    ));
    let second_owner_descriptor = CompileMethodDescriptor {
        owner: TypeId::new(299),
        debug_name: "Other::push".to_owned(),
        ..descriptor.clone()
    };
    snapshot
        .insert_method_descriptor(second_owner_descriptor.clone(), fact_origin)
        .expect("one trait MethodId may describe a distinct receiver owner");
    snapshot
        .insert_guard(guard_key, guard.clone(), fact_origin)
        .expect("guard target should be unique");
    snapshot
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: type_id,
                canonical_name: "game::Reward".to_owned(),
                class: CompileTypeClass::ScriptEnum,
                shape: None,
                fields: Vec::new(),
                variants: vec![variant_id],
            },
            fact_origin,
        )
        .expect("type descriptor should be unique");
    snapshot
        .insert_variant_descriptor(
            CompileVariantDescriptor {
                id: variant_id,
                owner: type_id,
                name: "Granted".to_owned(),
                fields: vec![field_id],
                declaration_order: 0,
            },
            fact_origin,
        )
        .expect("variant descriptor should be unique");
    snapshot
        .insert_field_descriptor(
            CompileFieldDescriptor {
                id: field_id,
                owner: type_id,
                variant: Some(variant_id),
                name: "amount".to_owned(),
                contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                declaration_order: 0,
                writable: true,
                host_runtime: None,
            },
            fact_origin,
        )
        .expect("field descriptor should be unique");
    snapshot
        .insert_global(
            global_declaration,
            CompileGlobalDescriptor {
                id: global_id,
                name: "current_reward".to_owned(),
                contract: MirTypeContract::Definition(type_id),
            },
            fact_origin,
        )
        .expect("global descriptor should be unique by declaration and stable ID");
    let snapshot = snapshot.build();

    assert_eq!(
        snapshot.method_descriptor(descriptor.owner, method),
        Some(&descriptor)
    );
    assert_eq!(
        snapshot.method_descriptor(second_owner_descriptor.owner, method),
        Some(&second_owner_descriptor)
    );
    assert_eq!(snapshot.guard(guard_key), Some(&guard));
    assert_eq!(
        snapshot.global(global_declaration),
        snapshot.global_by_id(global_id)
    );
    assert_eq!(
        snapshot
            .variant_descriptor(variant_id)
            .map(|value| value.name.as_str()),
        Some("Granted")
    );
    assert_eq!(
        snapshot
            .field_descriptor(field_id)
            .map(|value| value.name.as_str()),
        Some("amount")
    );
    let program = MirProgram::new(snapshot.target_table().clone());
    let dump = program.dump();
    assert!(dump.contains("Array::push"));
    assert!(dump.contains("game::Reward"));
    assert!(dump.contains("current_reward"));
    assert!(!dump.contains("corrupt"));
}

#[test]
fn mir_model_keeps_record_and_variant_field_slots_distinct() {
    let type_id = TypeId::new(223);
    let shape = vela_common::ShapeId::new(224);
    let variant = vela_def::VariantId::new(225);
    let field = vela_def::FieldId::new(226);

    assert!(matches!(
        CompileFieldTarget::RecordSlot {
            type_id,
            shape,
            field,
        },
        CompileFieldTarget::RecordSlot {
            type_id: actual_type,
            shape: actual_shape,
            field: actual_field,
        } if actual_type == type_id && actual_shape == shape && actual_field == field
    ));
    assert!(matches!(
        CompileFieldTarget::VariantSlot {
            type_id,
            variant,
            field,
        },
        CompileFieldTarget::VariantSlot {
            type_id: actual_type,
            variant: actual_variant,
            field: actual_field,
        } if actual_type == type_id && actual_variant == variant && actual_field == field
    ));
    assert!(matches!(
        MirFieldTarget::VariantSlot {
            type_id,
            variant,
            field,
        },
        MirFieldTarget::VariantSlot {
            type_id: actual_type,
            variant: actual_variant,
            field: actual_field,
        } if actual_type == type_id && actual_variant == variant && actual_field == field
    ));
}

#[test]
fn mir_model_materializes_heap_constants_at_explicit_safepoints() {
    let body = HirBodyId::new(23);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(230)),
        origin,
    );
    let entry = function.entry_block();
    let destination = function.add_temp(MirValueType::Primitive(PrimitiveTag::String), origin);
    let statement = MirStatement::new(
        origin,
        Some(MirPlace::temp(destination)),
        MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String("tick".to_owned())),
        MirEffect::PURE,
        None,
    );
    assert_eq!(
        function.append_statement(entry, statement.clone()),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::allocation(),
            actual: MirEffect::PURE,
        })
    );
    let statement = MirStatement {
        effect: MirEffect::allocation(),
        ..statement
    };
    assert_eq!(
        function.append_statement(entry, statement.clone()),
        Err(MirBuildError::MissingSafepoint { origin })
    );
    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    function
        .append_statement(
            entry,
            MirStatement {
                safepoint: Some(safepoint),
                ..statement
            },
        )
        .expect("heap-backed constants must be explicit allocation boundaries");

    let declaration = HirDeclId::new(231);
    let evaluated = MirEvaluatedConstant::Array(vec![
        MirEvaluatedConstant::Scalar(ScalarValue::I64(1)),
        MirEvaluatedConstant::Map(vec![(
            "name".to_owned(),
            MirEvaluatedConstant::String("vela".to_owned()),
        )]),
    ]);
    let mut snapshot = CompileTargetSnapshot::builder();
    snapshot
        .insert_evaluated_constant(declaration, evaluated.clone(), origin)
        .expect("evaluated const data should enter the immutable target snapshot once");
    assert_eq!(
        snapshot.insert_evaluated_constant(declaration, evaluated.clone(), origin),
        Err(MirBuildError::DuplicateEvaluatedConstant {
            declaration,
            origin,
        })
    );
    assert_eq!(
        snapshot.build().evaluated_constant(declaration),
        Some(&evaluated)
    );
}

#[test]
fn mir_model_owns_distinct_schema_defaults_and_resolved_constructor_slots() {
    let origin = origin(HirBodyId::new(232));
    let first_default = HirBodyId::new(233);
    let second_default = HirBodyId::new(234);
    let first_value = MirEvaluatedConstant::Scalar(ScalarValue::I64(10));
    let second_value = MirEvaluatedConstant::String("ready".to_owned());
    let type_id = TypeId::new(235);
    let shape = vela_common::ShapeId::new(236);
    let variant = vela_def::VariantId::new(237);
    let first_field = vela_def::FieldId::new(238);
    let second_field = vela_def::FieldId::new(239);
    let fields = vec![
        CompileConstructorField {
            field: first_field,
            parameter: 0,
            parameter_name: "quest_id".to_owned(),
            value: CompileConstructorValue::EvaluatedDefault(first_default),
        },
        CompileConstructorField {
            field: second_field,
            parameter: 1,
            parameter_name: "progress".to_owned(),
            value: CompileConstructorValue::EvaluatedDefault(second_default),
        },
    ];
    let record_expression = HirExprId::new(240);
    let variant_expression = HirExprId::new(241);
    let mut snapshot = CompileTargetSnapshot::builder();
    snapshot
        .insert_evaluated_schema_default(first_default, first_value.clone(), origin)
        .expect("first field default should retain its HIR body identity");
    snapshot
        .insert_evaluated_schema_default(second_default, second_value.clone(), origin)
        .expect("second field default should not collide with the first");
    snapshot
        .insert_constructor(
            record_expression,
            CompileConstructorTarget::Record {
                type_id,
                shape,
                fields: fields.clone(),
            },
            origin,
        )
        .expect("record constructor placement should be unique");
    snapshot
        .insert_constructor(
            variant_expression,
            CompileConstructorTarget::Variant {
                type_id,
                variant,
                fields: fields.clone(),
            },
            origin,
        )
        .expect("variant constructor placement should be unique");
    let snapshot = snapshot.build();

    assert_eq!(
        snapshot.evaluated_schema_default(first_default),
        Some(&first_value)
    );
    assert_eq!(
        snapshot.evaluated_schema_default(second_default),
        Some(&second_value)
    );
    assert!(matches!(
        snapshot.constructor(record_expression),
        Some(CompileConstructorTarget::Record { fields: actual, .. }) if actual == &fields
    ));
    assert!(matches!(
        snapshot.constructor(variant_expression),
        Some(CompileConstructorTarget::Variant { fields: actual, .. }) if actual == &fields
    ));
}

#[test]
fn mir_model_calls_encode_receivers_and_default_delivery_contracts() {
    let body = HirBodyId::new(24);
    let origin = origin(body);
    let mut function = test_function(
        body,
        MirFunctionOwner::Function(FunctionId::new(240)),
        origin,
    );
    let entry = function.entry_block();
    let receiver = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let method_result = function.add_temp(MirValueType::Dynamic, origin);
    let safepoint = function.add_safepoint(MirSafepoint::new(origin));
    let signature_effect = MirEffect::global_read();
    let script_signature = CompileSignature {
        parameters: vec![CompileParameter {
            name: "fallback".to_owned(),
            contract: None,
            default: CompileParameterDefault::HirBody(HirBodyId::new(241)),
            origin: Some(origin),
        }],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: signature_effect,
    };
    let method = MethodExecutableTarget {
        method: MethodId::new(242),
        function: FunctionId::new(243),
        owner: TypeId::new(244),
        node: HirNodeId::new(245),
    };
    let call = MirStatement::new(
        origin,
        Some(MirPlace::temp(method_result)),
        MirStatementKind::Call(MirCall::ScriptMethod {
            target: method,
            debug_name: "apply".to_owned(),
            receiver: MirOperand::Local(receiver),
            signature: script_signature,
            arguments: vec![MirScriptArgument::missing(0)],
        }),
        MirEffect::script_call(),
        Some(safepoint),
    );
    assert_eq!(
        function.append_statement(entry, call.clone()),
        Err(MirBuildError::IncompleteEffect {
            origin,
            required: MirEffect::script_call().union(signature_effect),
            actual: MirEffect::script_call(),
        })
    );
    function
        .append_statement(
            entry,
            MirStatement {
                effect: MirEffect::script_call().union(signature_effect),
                ..call
            },
        )
        .expect("script methods should retain an explicit receiver and callee default slot");

    let external_result = function.add_temp(MirValueType::Dynamic, origin);
    let external_signature = CompileSignature {
        parameters: vec![
            CompileParameter {
                name: "value".to_owned(),
                contract: None,
                default: CompileParameterDefault::Required,
                origin: None,
            },
            CompileParameter {
                name: "limit".to_owned(),
                contract: None,
                default: CompileParameterDefault::RuntimeProvided,
                origin: None,
            },
        ],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::PURE,
    };
    let external_call = MirStatement::new(
        origin,
        Some(MirPlace::temp(external_result)),
        MirStatementKind::Call(MirCall::NativeFunction {
            function: FunctionId::new(246),
            debug_name: "native::limit".to_owned(),
            signature: external_signature.clone(),
            arguments: Vec::new(),
        }),
        MirEffect::external_call(),
        Some(safepoint),
    );
    assert_eq!(
        function.append_statement(entry, external_call.clone()),
        Err(MirBuildError::InvalidCallArgumentPlacement { origin })
    );
    function
        .append_statement(
            entry,
            MirStatement {
                kind: MirStatementKind::Call(MirCall::NativeFunction {
                    function: FunctionId::new(246),
                    debug_name: "native::limit".to_owned(),
                    signature: external_signature.clone(),
                    arguments: vec![MirOperand::Immediate(MirImmediate::Unit)],
                }),
                ..external_call
            },
        )
        .expect("external defaults should be represented only as an omitted trailing suffix");

    let runtime_result = function.add_temp(MirValueType::Dynamic, origin);
    function
        .append_statement(
            entry,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(runtime_result)),
                MirStatementKind::Call(MirCall::NativeFunction {
                    function: FunctionId::new(247),
                    debug_name: "native::runtime_checked".to_owned(),
                    signature: CompileSignature {
                        positional: CompilePositionalPolicy::RuntimeChecked,
                        ..external_signature.clone()
                    },
                    arguments: vec![
                        MirOperand::Immediate(MirImmediate::Unit),
                        MirOperand::Immediate(MirImmediate::Unit),
                        MirOperand::Immediate(MirImmediate::Unit),
                    ],
                }),
                MirEffect::external_call(),
                Some(safepoint),
            ),
        )
        .expect("runtime-checked external calls retain all positional operands");

    let variadic_result = function.add_temp(MirValueType::Dynamic, origin);
    let variadic_call = MirStatement::new(
        origin,
        Some(MirPlace::temp(variadic_result)),
        MirStatementKind::Call(MirCall::NativeFunction {
            function: FunctionId::new(248),
            debug_name: "native::variadic".to_owned(),
            signature: CompileSignature {
                positional: CompilePositionalPolicy::Variadic { minimum: 2 },
                ..external_signature
            },
            arguments: vec![MirOperand::Immediate(MirImmediate::Unit)],
        }),
        MirEffect::external_call(),
        Some(safepoint),
    );
    assert_eq!(
        function.append_statement(entry, variadic_call.clone()),
        Err(MirBuildError::InvalidCallArgumentPlacement { origin })
    );
    function
        .append_statement(
            entry,
            MirStatement {
                kind: MirStatementKind::Call(MirCall::NativeFunction {
                    function: FunctionId::new(248),
                    debug_name: "native::variadic".to_owned(),
                    signature: CompileSignature {
                        parameters: Vec::new(),
                        positional: CompilePositionalPolicy::Variadic { minimum: 2 },
                        return_contract: None,
                        effect: MirEffect::PURE,
                    },
                    arguments: vec![
                        MirOperand::Immediate(MirImmediate::Unit),
                        MirOperand::Immediate(MirImmediate::Unit),
                    ],
                }),
                ..variadic_call
            },
        )
        .expect("proven variadic calls enforce only their minimum positional arity");
}
