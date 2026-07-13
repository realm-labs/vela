use vela_common::{ShapeId, SourceId, Span};
use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId};

use crate::*;

fn origin(seed: u32) -> MirSourceOrigin {
    MirSourceOrigin::expression(
        HirBodyId::new(seed),
        HirExprId::new(seed + 1),
        Span::new(SourceId::new(71), seed, seed + 9),
    )
}

fn parameter(name: &str, default: CompileParameterDefault) -> CompileParameter {
    CompileParameter {
        name: name.to_owned(),
        contract: None,
        default,
        origin: None,
    }
}

fn function_descriptor(
    function: FunctionId,
    class: CompileFunctionClass,
    parameters: Vec<CompileParameter>,
) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id: function,
        class,
        canonical_symbol: format!("test::function_{}", function.get()),
        debug_name: format!("function_{}", function.get()),
        signature: CompileSignature {
            asyncness: vela_common::CallableAsyncness::Sync,
            parameters,
            positional: CompilePositionalPolicy::RuntimeChecked,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileFunctionAccess::new(true, true, false),
    }
}

fn insert_root(
    builder: &mut CompileTargetSnapshotBuilder,
    function: FunctionId,
    origin: MirSourceOrigin,
) {
    let mut descriptor = function_descriptor(function, CompileFunctionClass::Script, Vec::new());
    descriptor.signature.positional = CompilePositionalPolicy::ExactOrTrailingDefaults;
    descriptor.access = CompileFunctionAccess::script(false);
    builder
        .insert_script_function(
            HirDeclId::new(hir_seed(function.get(), 1_000)),
            HirBodyId::new(hir_seed(function.get(), 2_000)),
            descriptor,
            origin,
        )
        .expect("call-order fixture root");
}

#[test]
fn placed_calls_keep_source_evaluation_order_separate_from_parameter_slots() {
    let caller = FunctionId::new(700);
    let script = FunctionId::new(701);
    let external = FunctionId::new(702);
    let origin = origin(703);
    let script_second = HirExprId::new(704);
    let script_first = HirExprId::new(705);
    let external_second = HirExprId::new(706);
    let external_first = HirExprId::new(707);
    let script_call = HirExprId::new(708);
    let external_call = HirExprId::new(709);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(&mut builder, caller, origin);

    let mut script_descriptor = function_descriptor(
        script,
        CompileFunctionClass::Script,
        vec![
            parameter("first", CompileParameterDefault::Required),
            parameter("second", CompileParameterDefault::Required),
            parameter(
                "third",
                CompileParameterDefault::HirBody(HirBodyId::new(710)),
            ),
        ],
    );
    script_descriptor.signature.positional = CompilePositionalPolicy::ExactOrTrailingDefaults;
    script_descriptor.access = CompileFunctionAccess::script(false);
    builder
        .insert_script_function_descriptor(HirDeclId::new(711), script_descriptor, origin)
        .expect("script callee descriptor");
    builder
        .insert_function_descriptor(
            function_descriptor(
                external,
                CompileFunctionClass::Native,
                vec![
                    parameter("first", CompileParameterDefault::Required),
                    parameter("second", CompileParameterDefault::Required),
                    parameter("third", CompileParameterDefault::RuntimeProvided),
                ],
            ),
            origin,
        )
        .expect("external callee descriptor");

    builder
        .insert_call(
            caller,
            script_call,
            CompileCallTarget::script(
                CompileCalleeTarget::ScriptFunction {
                    function: script,
                    debug_name: "script".to_owned(),
                },
                vec![script_second, script_first],
                vec![
                    CompilePlacedCallArgument::placed(0, 1, script_first),
                    CompilePlacedCallArgument::placed(1, 0, script_second),
                    CompilePlacedCallArgument::missing(2),
                ],
            ),
            origin,
        )
        .expect("script call placement");
    builder
        .insert_call(
            caller,
            external_call,
            CompileCallTarget::external_named(
                CompileCalleeTarget::NativeFunction {
                    function: external,
                    debug_name: "external".to_owned(),
                },
                vec![external_second, external_first],
                vec![
                    CompilePlacedCallArgument::placed(0, 1, external_first),
                    CompilePlacedCallArgument::placed(1, 0, external_second),
                    CompilePlacedCallArgument::missing(2),
                ],
            ),
            origin,
        )
        .expect("external call placement");
    let snapshot = builder
        .build()
        .expect("source-order and slot-order projections are consistent");

    assert_placed_call(
        &snapshot
            .call(caller, script_call)
            .expect("script call")
            .arguments,
        &[script_second, script_first],
        &[Some(1), Some(0), None],
    );
    assert_placed_call(
        &snapshot
            .call(caller, external_call)
            .expect("external call")
            .arguments,
        &[external_second, external_first],
        &[Some(1), Some(0), None],
    );
}

#[test]
fn placed_call_validation_rejects_lossy_or_incompatible_source_projections() {
    let first = HirExprId::new(720);
    let second = HirExprId::new(721);
    let cases = [
        (
            CompileCallTarget::external_named(
                native_callee(),
                vec![first, second],
                vec![
                    CompilePlacedCallArgument::placed(0, 0, first),
                    CompilePlacedCallArgument::placed(1, 0, first),
                ],
            ),
            "source index is reused",
        ),
        (
            CompileCallTarget::external_named(
                native_callee(),
                vec![first, second],
                vec![
                    CompilePlacedCallArgument::placed(0, 1, first),
                    CompilePlacedCallArgument::placed(1, 0, second),
                ],
            ),
            "source value disagrees with evaluation order",
        ),
        (
            CompileCallTarget::external_named(
                native_callee(),
                vec![first, second],
                vec![
                    CompilePlacedCallArgument::placed(0, 2, first),
                    CompilePlacedCallArgument::placed(1, 0, first),
                ],
            ),
            "source index is out of bounds",
        ),
        (
            CompileCallTarget::external_named(
                CompileCalleeTarget::Local(HirLocalId::new(722)),
                vec![first],
                vec![CompilePlacedCallArgument::placed(0, 0, first)],
            ),
            "cannot use placed external arguments",
        ),
    ];

    for (target, expected) in cases {
        let error = build_external_call(target, false)
            .expect_err("malformed placed call must fail snapshot closure");
        assert!(
            error.to_string().contains(expected),
            "expected {error:?} to contain {expected:?}"
        );
    }

    let missing_required = CompileCallTarget::external_named(
        native_callee(),
        vec![first],
        vec![
            CompilePlacedCallArgument::placed(0, 0, first),
            CompilePlacedCallArgument::missing(1),
        ],
    );
    let error = build_external_call(missing_required, false)
        .expect_err("a required external parameter cannot be a missing slot");
    assert!(
        error
            .to_string()
            .contains("external named call omits required parameter 1")
    );

    let uncovered = CompileCallTarget::external_named(
        native_callee(),
        vec![first, second],
        vec![
            CompilePlacedCallArgument::placed(0, 0, first),
            CompilePlacedCallArgument::missing(1),
        ],
    );
    let error = build_external_call(uncovered, true)
        .expect_err("unrepresented source expression must fail snapshot closure");
    assert!(
        error
            .to_string()
            .contains("evaluation order is not covered by parameter slots")
    );
}

#[test]
fn set_from_array_snapshot_requires_one_canonical_positional_operand() {
    let value = HirExprId::new(760);
    let extra = HirExprId::new(761);
    build_set_from_array_call(CompileCallTarget::positional(
        set_from_array_callee(),
        vec![value],
    ))
    .expect("one positional set source must validate");

    for (target, expected) in [
        (
            CompileCallTarget::positional(set_from_array_callee(), Vec::new()),
            "must own exactly one source operand",
        ),
        (
            CompileCallTarget::positional(set_from_array_callee(), vec![value, extra]),
            "must own exactly one source operand",
        ),
        (
            CompileCallTarget::external_named(
                set_from_array_callee(),
                vec![value],
                vec![CompilePlacedCallArgument::placed(0, 0, value)],
            ),
            "must use canonical positional arguments",
        ),
    ] {
        let error = build_set_from_array_call(target)
            .expect_err("non-canonical set source operands must fail snapshot closure");
        assert!(
            error.to_string().contains(expected),
            "expected {error:?} to contain {expected:?}"
        );
    }
}

#[test]
fn static_constructors_keep_source_order_separate_from_schema_field_order() {
    let caller = FunctionId::new(730);
    let record_type = TypeId::new(731);
    let record_shape = ShapeId::new(732);
    let record_first = FieldId::new(733);
    let record_second = FieldId::new(734);
    let enum_type = TypeId::new(735);
    let variant = VariantId::new(736);
    let variant_first = FieldId::new(737);
    let variant_second = FieldId::new(738);
    let second_value = HirExprId::new(739);
    let first_value = HirExprId::new(740);
    let record_expression = HirExprId::new(741);
    let variant_expression = HirExprId::new(742);
    let origin = origin(743);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(&mut builder, caller, origin);
    insert_record_type(
        &mut builder,
        record_type,
        record_shape,
        [record_first, record_second],
        origin,
    );
    insert_variant_type(
        &mut builder,
        enum_type,
        variant,
        [variant_first, variant_second],
        origin,
    );
    builder
        .insert_constructor(
            caller,
            record_expression,
            CompileConstructorTarget::Record {
                type_id: record_type,
                shape: record_shape,
                evaluation_order: vec![second_value, first_value],
                fields: explicit_fields([record_first, record_second], first_value, second_value),
            },
            origin,
        )
        .expect("record constructor placement");
    builder
        .insert_constructor(
            caller,
            variant_expression,
            CompileConstructorTarget::Variant {
                type_id: enum_type,
                variant,
                evaluation_order: vec![second_value, first_value],
                fields: explicit_fields([variant_first, variant_second], first_value, second_value),
            },
            origin,
        )
        .expect("variant constructor placement");
    let snapshot = builder
        .build()
        .expect("constructor source and schema projections are consistent");

    for expression in [record_expression, variant_expression] {
        let target = snapshot
            .constructor(caller, expression)
            .expect("static constructor target");
        let (evaluation_order, fields) = match target {
            CompileConstructorTarget::Record {
                evaluation_order,
                fields,
                ..
            }
            | CompileConstructorTarget::Variant {
                evaluation_order,
                fields,
                ..
            } => (evaluation_order, fields),
            CompileConstructorTarget::DynamicRecord { .. }
            | CompileConstructorTarget::DynamicVariant { .. } => {
                panic!("fixture must remain statically placed")
            }
        };
        assert_eq!(evaluation_order, &[second_value, first_value]);
        assert_eq!(
            fields
                .iter()
                .map(|field| match field.value {
                    CompileConstructorValue::Explicit { source_index, .. } => Some(source_index),
                    CompileConstructorValue::EvaluatedDefault(_) => None,
                })
                .collect::<Vec<_>>(),
            [Some(1), Some(0)]
        );
    }
}

#[test]
fn static_constructor_validation_rejects_lossy_source_projections() {
    let fields = [FieldId::new(744), FieldId::new(745)];
    let second = HirExprId::new(746);
    let first = HirExprId::new(747);
    let mut duplicate_source = explicit_fields(fields, first, second);
    duplicate_source[1].value = CompileConstructorValue::Explicit {
        source_index: 1,
        value: first,
    };
    let mut wrong_schema_order = explicit_fields(fields, first, second);
    wrong_schema_order.swap(0, 1);

    for (placed, expected) in [
        (duplicate_source, "constructor source index is reused"),
        (
            wrong_schema_order,
            "constructor fields are not in contiguous descriptor order",
        ),
    ] {
        let error = build_record_constructor(vec![second, first], placed)
            .expect_err("lossy constructor placement must fail snapshot closure");
        assert!(
            error.to_string().contains(expected),
            "expected {error:?} to contain {expected:?}"
        );
    }
}

fn native_callee() -> CompileCalleeTarget {
    CompileCalleeTarget::NativeFunction {
        function: FunctionId::new(750),
        debug_name: "external".to_owned(),
    }
}

fn set_from_array_callee() -> CompileCalleeTarget {
    CompileCalleeTarget::SetFromArray {
        function: FunctionId::new(762),
        debug_name: "set::from_array".to_owned(),
    }
}

fn build_set_from_array_call(
    target: CompileCallTarget,
) -> Result<CompileTargetSnapshot, MirBuildError> {
    let caller = FunctionId::new(763);
    let origin = origin(764);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(&mut builder, caller, origin);
    builder.insert_function_descriptor(
        function_descriptor(
            FunctionId::new(762),
            CompileFunctionClass::Stdlib,
            vec![parameter("values", CompileParameterDefault::Required)],
        ),
        origin,
    )?;
    builder.insert_call(caller, HirExprId::new(765), target, origin)?;
    builder.build()
}

fn build_external_call(
    target: CompileCallTarget,
    second_defaulted: bool,
) -> Result<CompileTargetSnapshot, MirBuildError> {
    let caller = FunctionId::new(751);
    let origin = origin(752);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(&mut builder, caller, origin);
    builder.insert_function_descriptor(
        function_descriptor(
            FunctionId::new(750),
            CompileFunctionClass::Native,
            vec![
                parameter("first", CompileParameterDefault::Required),
                parameter(
                    "second",
                    if second_defaulted {
                        CompileParameterDefault::RuntimeProvided
                    } else {
                        CompileParameterDefault::Required
                    },
                ),
            ],
        ),
        origin,
    )?;
    builder.insert_call(caller, HirExprId::new(753), target, origin)?;
    builder.build()
}

fn build_record_constructor(
    evaluation_order: Vec<HirExprId>,
    fields: Vec<CompileConstructorField>,
) -> Result<CompileTargetSnapshot, MirBuildError> {
    let caller = FunctionId::new(754);
    let type_id = TypeId::new(755);
    let shape = ShapeId::new(756);
    let origin = origin(757);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(&mut builder, caller, origin);
    insert_record_type(
        &mut builder,
        type_id,
        shape,
        [FieldId::new(744), FieldId::new(745)],
        origin,
    );
    builder.insert_constructor(
        caller,
        HirExprId::new(758),
        CompileConstructorTarget::Record {
            type_id,
            shape,
            evaluation_order,
            fields,
        },
        origin,
    )?;
    builder.build()
}

fn assert_placed_call(
    arguments: &CompileCallArguments,
    expected_order: &[HirExprId],
    expected_sources: &[Option<u32>],
) {
    let (evaluation_order, parameter_slots) = match arguments {
        CompileCallArguments::Script {
            evaluation_order,
            parameter_slots,
        }
        | CompileCallArguments::ExternalNamed {
            evaluation_order,
            parameter_slots,
        } => (evaluation_order, parameter_slots),
        CompileCallArguments::Positional(_) | CompileCallArguments::Dynamic(_) => {
            panic!("expected a statically placed call")
        }
    };
    assert_eq!(evaluation_order, expected_order);
    assert_eq!(
        parameter_slots
            .iter()
            .map(|slot| match slot.value {
                CompilePlacedCallValue::Explicit { source_index, .. } => Some(source_index),
                CompilePlacedCallValue::MissingDefault => None,
            })
            .collect::<Vec<_>>(),
        expected_sources
    );
}

fn insert_record_type(
    builder: &mut CompileTargetSnapshotBuilder,
    type_id: TypeId,
    shape: ShapeId,
    fields: [FieldId; 2],
    origin: MirSourceOrigin,
) {
    builder
        .insert_script_type(
            HirDeclId::new(hir_seed(type_id.get(), 1_000)),
            CompileTypeDescriptor {
                id: type_id,
                canonical_name: "test::Record".to_owned(),
                runtime_name: "test::Record".to_owned(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(shape),
                fields: fields.to_vec(),
                variants: Vec::new(),
            },
            origin,
        )
        .expect("record type descriptor");
    insert_fields(builder, type_id, None, fields, origin);
}

fn insert_variant_type(
    builder: &mut CompileTargetSnapshotBuilder,
    type_id: TypeId,
    variant: VariantId,
    fields: [FieldId; 2],
    origin: MirSourceOrigin,
) {
    builder
        .insert_script_type(
            HirDeclId::new(hir_seed(type_id.get(), 1_000)),
            CompileTypeDescriptor {
                id: type_id,
                canonical_name: "test::Enum".to_owned(),
                runtime_name: "test::Enum".to_owned(),
                class: CompileTypeClass::ScriptEnum,
                shape: None,
                fields: Vec::new(),
                variants: vec![variant],
            },
            origin,
        )
        .expect("enum type descriptor");
    builder
        .insert_variant_descriptor(
            CompileVariantDescriptor {
                id: variant,
                owner: type_id,
                name: "Case".to_owned(),
                fields: fields.to_vec(),
                declaration_order: 0,
            },
            origin,
        )
        .expect("variant descriptor");
    insert_fields(builder, type_id, Some(variant), fields, origin);
}

fn insert_fields(
    builder: &mut CompileTargetSnapshotBuilder,
    owner: TypeId,
    variant: Option<VariantId>,
    fields: [FieldId; 2],
    origin: MirSourceOrigin,
) {
    for (index, (field, name)) in fields.into_iter().zip(["first", "second"]).enumerate() {
        builder
            .insert_field_descriptor(
                CompileFieldDescriptor {
                    id: field,
                    owner,
                    variant,
                    name: name.to_owned(),
                    contract: None,
                    declaration_order: u32::try_from(index).expect("two fields fit in u32"),
                    access: CompileFieldAccess::script(),
                    host_runtime: None,
                },
                origin,
            )
            .expect("field descriptor");
    }
}

fn explicit_fields(
    fields: [FieldId; 2],
    first_value: HirExprId,
    second_value: HirExprId,
) -> Vec<CompileConstructorField> {
    vec![
        CompileConstructorField {
            field: fields[0],
            parameter: 0,
            parameter_name: "first".to_owned(),
            value: CompileConstructorValue::Explicit {
                source_index: 1,
                value: first_value,
            },
        },
        CompileConstructorField {
            field: fields[1],
            parameter: 1,
            parameter_name: "second".to_owned(),
            value: CompileConstructorValue::Explicit {
                source_index: 0,
                value: second_value,
            },
        },
    ]
}

fn hir_seed(stable: u128, offset: u32) -> u32 {
    u32::try_from(stable)
        .expect("fixture stable ID fits in u32")
        .checked_add(offset)
        .expect("fixture HIR seed fits in u32")
}
