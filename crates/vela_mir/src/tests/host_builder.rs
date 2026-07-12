use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_common::{HostMethodId, HostTypeId, PrimitiveTag, ShapeId, SourceId};
use vela_def::{FieldId, FunctionId, MethodId, TypeId};
use vela_hir::body::{HirBody, HirExprKind, HirLiteral};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::{
    CompileCallTarget, CompileCalleeTarget, CompileFieldAccess, CompileFieldDescriptor,
    CompileFunctionAccess, CompileFunctionClass, CompileFunctionDescriptor,
    CompileFunctionIdentity, CompileGuardKey, CompileGuardTarget, CompileHostIndexCapability,
    CompileHostPathSegment, CompileHostPathTarget, CompileMemberTarget, CompileMethodAccess,
    CompileMethodClass, CompileMethodDescriptor, CompileParameter, CompileParameterDefault,
    CompilePositionalPolicy, CompileSignature, CompileTargetSnapshot, CompileTargetSnapshotBuilder,
    CompileTypeClass, CompileTypeDescriptor, HostFieldTarget, HostMethodTarget, HostTypeTarget,
    MirBuildError, MirEffect, MirEvaluatedConstant, MirGuardLocation, MirHostMutation,
    MirHostOperation, MirHostPathSegment, MirLoweringConfig, MirLoweringInput, MirOperand,
    MirPlace, MirProgram, MirSourceOrigin, MirStatementKind, MirTypeContract, MirValueType,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(9_100);
const ROOT_TYPE_ID: TypeId = TypeId::new(9_101);
const ROOT_RUNTIME_ID: HostTypeId = HostTypeId::new(9_102);
const COLLECTION_TYPE_ID: TypeId = TypeId::new(9_103);
const COLLECTION_RUNTIME_ID: HostTypeId = HostTypeId::new(9_104);
const ITEM_TYPE_ID: TypeId = TypeId::new(9_105);
const ITEM_RUNTIME_ID: HostTypeId = HostTypeId::new(9_106);
const ITEMS_FIELD: FieldId = FieldId::new(9_107);
const AMOUNT_FIELD: FieldId = FieldId::new(9_108);
const GRANT_METHOD: MethodId = MethodId::new(9_109);
const GRANT_RUNTIME: HostMethodId = HostMethodId::new(9_110);
const TOUCH_METHOD: MethodId = MethodId::new(9_111);
const TOUCH_RUNTIME: HostMethodId = HostMethodId::new(9_112);
const SCRIPT_RECORD_TYPE_ID: TypeId = TypeId::new(9_113);
const SCRIPT_RECORD_SHAPE: ShapeId = ShapeId::new(9_114);
const SCRIPT_ENUM_TYPE_ID: TypeId = TypeId::new(9_115);
const RECORD_FIELD: FieldId = FieldId::new(9_116);
const STATE_FIELD: FieldId = FieldId::new(9_117);
const NOTHING_FIELD: FieldId = FieldId::new(9_118);
const MIX_METHOD: MethodId = MethodId::new(9_119);
const MIX_RUNTIME: HostMethodId = HostMethodId::new(9_120);

const ROOT_TYPE: HostTypeTarget = HostTypeTarget {
    semantic: ROOT_TYPE_ID,
    runtime: ROOT_RUNTIME_ID,
};
const COLLECTION_TYPE: HostTypeTarget = HostTypeTarget {
    semantic: COLLECTION_TYPE_ID,
    runtime: COLLECTION_RUNTIME_ID,
};
const ITEM_TYPE: HostTypeTarget = HostTypeTarget {
    semantic: ITEM_TYPE_ID,
    runtime: ITEM_RUNTIME_ID,
};

fn build_host(source: &str) -> Result<MirProgram, MirBuildError> {
    build_host_with_configuration(source, |_, _| {}, |_, _| Ok(()))
}

fn build_host_with_path_mutator(
    source: &str,
    mutate_path: impl FnMut(HirExprId, &mut CompileHostPathTarget),
) -> Result<MirProgram, MirBuildError> {
    build_host_with_configuration(source, mutate_path, |_, _| Ok(()))
}

fn build_host_with_configuration(
    source: &str,
    mut mutate_path: impl FnMut(HirExprId, &mut CompileHostPathTarget),
    configure: impl FnOnce(&HirBody, &mut CompileTargetSnapshotBuilder) -> Result<(), MirBuildError>,
) -> Result<MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(91),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("host_builder"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let body_origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id)],
    )
    .expect("host builder analysis");

    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        function_descriptor(&graph, body),
        body_origin,
    )?;
    insert_host_descriptors(&mut targets, body_origin)?;
    insert_host_placements(body, &mut targets, &mut mutate_path)?;
    configure(body, &mut targets)?;
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(ROOT_FUNCTION),
        body.id,
        analysis.view(ROOT_FUNCTION).expect("root analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;
    crate::build_mir(input)
}

fn function_descriptor(graph: &ModuleGraph, body: &HirBody) -> CompileFunctionDescriptor {
    let bindings = graph
        .bindings_for_body(body.id)
        .expect("host fixture bindings");
    let parameters = body
        .params
        .iter()
        .map(|parameter| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone(),
            contract: None,
            default: CompileParameterDefault::Required,
            origin: None,
        })
        .collect();
    CompileFunctionDescriptor {
        id: ROOT_FUNCTION,
        class: CompileFunctionClass::Script,
        canonical_symbol: "host_builder::main".to_owned(),
        debug_name: "main".to_owned(),
        signature: CompileSignature {
            parameters,
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileFunctionAccess::script(false),
    }
}

fn insert_host_descriptors(
    targets: &mut CompileTargetSnapshotBuilder,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    for descriptor in [
        CompileTypeDescriptor {
            id: ROOT_TYPE_ID,
            canonical_name: "host::Root".to_owned(),
            runtime_name: "Root".to_owned(),
            class: CompileTypeClass::Host {
                runtime: ROOT_RUNTIME_ID,
            },
            shape: None,
            fields: vec![ITEMS_FIELD, RECORD_FIELD, STATE_FIELD, NOTHING_FIELD],
            variants: Vec::new(),
        },
        CompileTypeDescriptor {
            id: COLLECTION_TYPE_ID,
            canonical_name: "host::Collection".to_owned(),
            runtime_name: "Collection".to_owned(),
            class: CompileTypeClass::Host {
                runtime: COLLECTION_RUNTIME_ID,
            },
            shape: None,
            fields: Vec::new(),
            variants: Vec::new(),
        },
        CompileTypeDescriptor {
            id: SCRIPT_RECORD_TYPE_ID,
            canonical_name: "script::Record".to_owned(),
            runtime_name: "Record".to_owned(),
            class: CompileTypeClass::ScriptRecord,
            shape: Some(SCRIPT_RECORD_SHAPE),
            fields: Vec::new(),
            variants: Vec::new(),
        },
        CompileTypeDescriptor {
            id: SCRIPT_ENUM_TYPE_ID,
            canonical_name: "script::State".to_owned(),
            runtime_name: "State".to_owned(),
            class: CompileTypeClass::ScriptEnum,
            shape: None,
            fields: Vec::new(),
            variants: Vec::new(),
        },
        CompileTypeDescriptor {
            id: ITEM_TYPE_ID,
            canonical_name: "host::Item".to_owned(),
            runtime_name: "Item".to_owned(),
            class: CompileTypeClass::Host {
                runtime: ITEM_RUNTIME_ID,
            },
            shape: None,
            fields: vec![AMOUNT_FIELD],
            variants: Vec::new(),
        },
    ] {
        targets.insert_type_descriptor(descriptor, origin)?;
    }
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: ITEMS_FIELD,
            owner: ROOT_TYPE_ID,
            variant: None,
            name: "items".to_owned(),
            contract: Some(MirTypeContract::Host(COLLECTION_TYPE)),
            declaration_order: 0,
            access: host_field_access(),
            host_runtime: Some(ITEMS_FIELD),
        },
        origin,
    )?;
    for (id, name, contract, declaration_order) in [
        (
            RECORD_FIELD,
            "record",
            MirTypeContract::Definition(SCRIPT_RECORD_TYPE_ID),
            1,
        ),
        (
            STATE_FIELD,
            "state",
            MirTypeContract::Definition(SCRIPT_ENUM_TYPE_ID),
            2,
        ),
        (
            NOTHING_FIELD,
            "nothing",
            MirTypeContract::Primitive(PrimitiveTag::Unit),
            3,
        ),
    ] {
        targets.insert_field_descriptor(
            CompileFieldDescriptor {
                id,
                owner: ROOT_TYPE_ID,
                variant: None,
                name: name.to_owned(),
                contract: Some(contract),
                declaration_order,
                access: host_field_access(),
                host_runtime: Some(id),
            },
            origin,
        )?;
    }
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: AMOUNT_FIELD,
            owner: ITEM_TYPE_ID,
            variant: None,
            name: "amount".to_owned(),
            contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            declaration_order: 0,
            access: host_field_access(),
            host_runtime: Some(AMOUNT_FIELD),
        },
        origin,
    )?;
    targets.insert_method_descriptor(
        CompileMethodDescriptor {
            id: GRANT_METHOD,
            owner: ITEM_TYPE_ID,
            member_name: "grant".to_owned(),
            debug_name: "host::Item::grant".to_owned(),
            class: CompileMethodClass::Host {
                runtime: GRANT_RUNTIME,
            },
            signature: grant_signature(),
            access: host_method_access(),
        },
        origin,
    )?;
    targets.insert_method_descriptor(
        CompileMethodDescriptor {
            id: TOUCH_METHOD,
            owner: ITEM_TYPE_ID,
            member_name: "touch".to_owned(),
            debug_name: "host::Item::touch".to_owned(),
            class: CompileMethodClass::Host {
                runtime: TOUCH_RUNTIME,
            },
            signature: touch_signature(),
            access: host_method_access(),
        },
        origin,
    )?;
    targets.insert_method_descriptor(
        CompileMethodDescriptor {
            id: MIX_METHOD,
            owner: ITEM_TYPE_ID,
            member_name: "mix".to_owned(),
            debug_name: "host::Item::mix".to_owned(),
            class: CompileMethodClass::Host {
                runtime: MIX_RUNTIME,
            },
            signature: mix_signature(),
            access: host_method_access(),
        },
        origin,
    )?;
    Ok(())
}

fn insert_host_placements(
    body: &HirBody,
    targets: &mut CompileTargetSnapshotBuilder,
    mutate_path: &mut impl FnMut(HirExprId, &mut CompileHostPathTarget),
) -> Result<(), MirBuildError> {
    for expression in body.expressions.values() {
        let origin = expression_origin(body, expression.id);
        if let Some(mut path) = fixture_host_path(body, expression.id) {
            mutate_path(expression.id, &mut path);
            targets.insert_host_path(ROOT_FUNCTION, expression.id, path, origin)?;
        }
        if let HirExprKind::Field(field) = &expression.kind
            && let Some(target) = field_target(&field.name)
        {
            targets.insert_member(
                ROOT_FUNCTION,
                expression.id,
                CompileMemberTarget::HostField(target),
                origin,
            )?;
        }
    }

    for expression in body.expressions.values() {
        let origin = expression_origin(body, expression.id);
        match &expression.kind {
            HirExprKind::Assign {
                target: Some(target),
                ..
            } => {
                let Some(mut path) = fixture_host_path(body, *target) else {
                    continue;
                };
                mutate_path(*target, &mut path);
                targets.insert_host_path(ROOT_FUNCTION, expression.id, path, origin)?;
            }
            HirExprKind::Call(call) => {
                let field = body.field(call.callee).expect("fixture call field");
                let mut path = fixture_host_path(body, field.receiver)
                    .expect("fixture host call receiver path");
                mutate_path(field.receiver, &mut path);
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| argument.value.expect("fixture call argument"))
                    .collect::<Vec<_>>();
                let callee = match field.name.as_str() {
                    "grant" => CompileCalleeTarget::HostMethod(host_method_target()),
                    "touch" => CompileCalleeTarget::HostMethod(touch_method_target()),
                    "mix" => CompileCalleeTarget::HostMethod(mix_method_target()),
                    "remove" => CompileCalleeTarget::HostRemove { path },
                    "push" => CompileCalleeTarget::HostPush { path },
                    name => panic!("unexpected host fixture call {name:?}"),
                };
                targets.insert_call(
                    ROOT_FUNCTION,
                    expression.id,
                    CompileCallTarget::positional(callee, arguments),
                    origin,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn fixture_host_path(body: &HirBody, expression: HirExprId) -> Option<CompileHostPathTarget> {
    let record = body.expression(expression)?;
    match &record.kind {
        HirExprKind::Path(path) => {
            let path = body.paths.get(path)?;
            matches!(path.path.as_slice(), [name] if name == "host" || name == "lookup").then_some(
                CompileHostPathTarget {
                    root: expression,
                    root_type: ROOT_TYPE,
                    segments: Vec::new(),
                },
            )
        }
        HirExprKind::Paren {
            expression: Some(inner),
        } => fixture_host_path(body, *inner),
        HirExprKind::Field(field) => {
            let mut path = fixture_host_path(body, field.receiver)?;
            path.segments
                .push(CompileHostPathSegment::Field(field_target(&field.name)?));
            Some(path)
        }
        HirExprKind::Index(index) => {
            let mut path = fixture_host_path(body, index.receiver)?;
            let segment = match body.expression(index.index).map(|value| &value.kind) {
                Some(HirExprKind::Literal(HirLiteral::Integer(value))) if value.text == "1" => {
                    CompileHostPathSegment::ConstantIndex {
                        value: 1,
                        capability: index_capability(),
                    }
                }
                _ => CompileHostPathSegment::DynamicIndex {
                    expression: index.index,
                    capability: index_capability(),
                },
            };
            path.segments.push(segment);
            Some(path)
        }
        _ => None,
    }
}

fn field_target(name: &str) -> Option<HostFieldTarget> {
    match name {
        "items" => Some(HostFieldTarget {
            owner: ROOT_TYPE,
            semantic: ITEMS_FIELD,
            runtime: ITEMS_FIELD,
            access: host_field_access(),
        }),
        "amount" => Some(HostFieldTarget {
            owner: ITEM_TYPE,
            semantic: AMOUNT_FIELD,
            runtime: AMOUNT_FIELD,
            access: host_field_access(),
        }),
        "record" => Some(HostFieldTarget {
            owner: ROOT_TYPE,
            semantic: RECORD_FIELD,
            runtime: RECORD_FIELD,
            access: host_field_access(),
        }),
        "state" => Some(HostFieldTarget {
            owner: ROOT_TYPE,
            semantic: STATE_FIELD,
            runtime: STATE_FIELD,
            access: host_field_access(),
        }),
        "nothing" => Some(HostFieldTarget {
            owner: ROOT_TYPE,
            semantic: NOTHING_FIELD,
            runtime: NOTHING_FIELD,
            access: host_field_access(),
        }),
        _ => None,
    }
}

fn index_capability() -> CompileHostIndexCapability {
    CompileHostIndexCapability {
        readable: true,
        writable: true,
        mutable: true,
        removable: true,
        key: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        value: Some(MirTypeContract::Host(ITEM_TYPE)),
    }
}

fn host_method_target() -> HostMethodTarget {
    HostMethodTarget {
        owner: ITEM_TYPE,
        semantic: GRANT_METHOD,
        runtime: GRANT_RUNTIME,
        signature: grant_signature(),
        access: host_method_access(),
    }
}

fn touch_method_target() -> HostMethodTarget {
    HostMethodTarget {
        owner: ITEM_TYPE,
        semantic: TOUCH_METHOD,
        runtime: TOUCH_RUNTIME,
        signature: touch_signature(),
        access: host_method_access(),
    }
}

fn mix_method_target() -> HostMethodTarget {
    HostMethodTarget {
        owner: ITEM_TYPE,
        semantic: MIX_METHOD,
        runtime: MIX_RUNTIME,
        signature: mix_signature(),
        access: host_method_access(),
    }
}

fn grant_signature() -> CompileSignature {
    CompileSignature {
        parameters: vec![CompileParameter {
            name: "value".to_owned(),
            contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            default: CompileParameterDefault::Required,
            origin: None,
        }],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        effect: MirEffect::host_write(),
    }
}

fn touch_signature() -> CompileSignature {
    CompileSignature {
        parameters: Vec::new(),
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::host_read(),
    }
}

fn mix_signature() -> CompileSignature {
    CompileSignature {
        parameters: vec![
            CompileParameter {
                name: "first".to_owned(),
                contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                default: CompileParameterDefault::Required,
                origin: None,
            },
            CompileParameter {
                name: "second".to_owned(),
                contract: None,
                default: CompileParameterDefault::Required,
                origin: None,
            },
        ],
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::host_write(),
    }
}

fn host_field_access() -> CompileFieldAccess {
    CompileFieldAccess::new(
        true,
        true,
        true,
        true,
        vec!["state.write".to_owned(), "state.read".to_owned()],
    )
}

fn host_method_access() -> CompileMethodAccess {
    CompileMethodAccess::new(true, true, vec!["state.call".to_owned()])
}

#[test]
fn mir_builder_lowers_every_hostaccess_operation_with_effects_and_safepoints() {
    let source = r#"
fn main(host, key, rhs) {
    let value = host.items[key].amount;
    host.items[key].amount = rhs;
    host.items[key].amount += rhs;
    host.items.push(rhs);
    host.items[key].remove();
    return host.items[key].grant(value);
}
"#;
    let program = build_host(source).expect("complete HostAccess function should lower");
    let function = only_function(&program);
    let host = function
        .statements()
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Host(operation) => Some((statement, operation)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(host.len(), 6, "{}", program.dump());
    assert!(matches!(host[0].1, MirHostOperation::Read { .. }));
    assert!(matches!(host[1].1, MirHostOperation::Write { .. }));
    assert!(matches!(
        host[2].1,
        MirHostOperation::Mutate {
            operation: MirHostMutation::Add,
            ..
        }
    ));
    assert!(matches!(
        host[3].1,
        MirHostOperation::Mutate {
            operation: MirHostMutation::Push,
            ..
        }
    ));
    assert!(matches!(host[4].1, MirHostOperation::Remove { .. }));
    assert!(matches!(host[5].1, MirHostOperation::Call { .. }));

    assert!(host[0].0.effect.host_read && host[0].0.effect.may_allocate);
    assert!(host[0].0.safepoint.is_some());
    for (statement, _) in &host[1..5] {
        assert!(statement.effect.host_read && statement.effect.host_write);
        assert!(!statement.effect.host_call);
        assert!(statement.safepoint.is_none());
        assert_eq!(statement.destination, None);
    }
    assert!(host[5].0.effect.host_call && host[5].0.effect.host_write);
    assert!(host[5].0.safepoint.is_some());
    assert!(matches!(host[5].0.destination, Some(MirPlace::Temp(_))));
    let call_destination = match host[5].0.destination {
        Some(MirPlace::Temp(temp)) => temp,
        destination => panic!("host call destination is not a temp: {destination:?}"),
    };
    assert_eq!(
        function
            .temp(call_destination)
            .expect("host call temp")
            .value_type,
        MirValueType::Primitive(PrimitiveTag::I64)
    );

    let read_destination = match host[0].0.destination {
        Some(MirPlace::Temp(temp)) => temp,
        destination => panic!("host read destination is not a temp: {destination:?}"),
    };
    assert_eq!(
        function
            .temp(read_destination)
            .expect("host read temp")
            .value_type,
        MirValueType::Primitive(PrimitiveTag::I64)
    );
    assert!(!function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::ReadField { .. }
            | MirStatementKind::WriteField { .. }
            | MirStatementKind::Index(_)
    )));
}

#[test]
fn host_argument_guard_traps_before_a_later_allocating_argument() {
    let source = r#"
fn main(host, first) {
    return host.items[1].mix(first, "later");
}
"#;
    let program = build_host_with_configuration(
        source,
        |_, _| {},
        |body, targets| {
            let first = body
                .expressions
                .values()
                .find_map(|expression| {
                    let HirExprKind::Call(call) = &expression.kind else {
                        return None;
                    };
                    let field = body.field(call.callee)?;
                    (field.name == "mix")
                        .then(|| call.arguments[0].value.expect("first host argument"))
                })
                .expect("mix call");
            targets.insert_guard(
                CompileGuardKey::Expression {
                    function: ROOT_FUNCTION,
                    expression: first,
                },
                CompileGuardTarget::new(
                    MirTypeContract::Primitive(PrimitiveTag::I64),
                    MirGuardLocation::Parameter { index: 0 },
                    "first",
                ),
                expression_origin(body, first),
            )
        },
    )
    .expect("guarded host call");
    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let guard = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
        .expect("first argument guard");
    let later = statements
        .iter()
        .position(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value))
                    if value == "later"
            )
        })
        .expect("later argument allocation");
    let call = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Host(MirHostOperation::Call { .. })
            )
        })
        .expect("host call");
    assert!(guard < later && later < call, "{}", program.dump());
    assert_eq!(function.guards().count(), 1);
}

#[test]
fn mir_builder_routes_host_mutation_before_ordinary_operator_selection() {
    let source = r#"
fn main(host, key, rhs) {
    host.items[key].amount += rhs;
}
"#;
    assert_eq!(
        assignment_operator_target(source),
        OperatorTargetFact::Dynamic,
        "the schema-free fixture deliberately has no proven script operator"
    );
    let program = build_host(source).expect("host mutation should not need a script operator mode");
    assert!(
        program
            .functions()
            .any(
                |(_, function)| function.statements().any(|(_, statement)| matches!(
                    statement.kind,
                    MirStatementKind::Host(MirHostOperation::Mutate {
                        operation: MirHostMutation::Add,
                        ..
                    })
                ))
            )
    );
}

#[test]
fn mir_builder_rejects_constant_host_index_values_that_disagree_with_hir() {
    let error = build_host_with_path_mutator(
        "fn main(host) { return host.items[1].amount; }",
        |_, path| {
            for segment in &mut path.segments {
                if let CompileHostPathSegment::ConstantIndex { value, .. } = segment {
                    *value = 2;
                }
            }
        },
    )
    .expect_err("forged constant host index must not erase the HIR literal");
    let MirBuildError::InconsistentInput { message, .. } = error else {
        panic!("unexpected constant-index error: {error:?}")
    };
    assert_eq!(
        message,
        "constant host index value disagrees with its HIR literal"
    );
}

#[test]
fn mir_builder_rejects_constant_host_indexes_attached_to_effectful_hir() {
    let error = build_host_with_path_mutator(
        "fn main(host, key) { return host.items[key = 1].amount; }",
        |_, path| {
            for segment in &mut path.segments {
                if let CompileHostPathSegment::DynamicIndex { capability, .. } = segment {
                    *segment = CompileHostPathSegment::ConstantIndex {
                        value: 1,
                        capability: capability.clone(),
                    };
                }
            }
        },
    )
    .expect_err("constant host placement must not skip an effectful HIR index");
    let MirBuildError::InconsistentInput { message, .. } = error else {
        panic!("unexpected nonliteral-index error: {error:?}")
    };
    assert_eq!(
        message,
        "constant host index placement is not attached to an integer literal"
    );
}

#[test]
fn mir_builder_rejects_constant_host_key_values_that_disagree_with_hir() {
    let error = build_host_with_path_mutator(
        "fn main(host) { return host.items[\"actual\"].amount; }",
        |_, path| {
            for segment in &mut path.segments {
                if let CompileHostPathSegment::DynamicIndex { capability, .. } = segment {
                    *segment = CompileHostPathSegment::ConstantKey {
                        value: "forged".to_owned(),
                        capability: capability.clone(),
                    };
                }
            }
        },
    )
    .expect_err("forged constant host key must not erase the HIR literal");
    let MirBuildError::InconsistentInput { message, .. } = error else {
        panic!("unexpected constant-key error: {error:?}")
    };
    assert_eq!(
        message,
        "constant host key value disagrees with its HIR literal"
    );
}

#[test]
fn mir_builder_validates_parenthesized_host_path_prefixes() {
    let program = build_host("fn main(host, key) { return (host.items[key]).amount; }")
        .expect("parenthesized host receiver should keep its exact inner placement");
    assert!(only_function(&program).statements().any(|(_, statement)| {
        matches!(
            statement.kind,
            MirStatementKind::Host(MirHostOperation::Read { .. })
        )
    }));
}

#[test]
fn mir_builder_does_not_infer_an_uncontracted_host_call_result_from_its_receiver() {
    let source = r#"
fn main(host) {
    return host.items[1].touch();
}
"#;
    let program = build_host(source).expect("uncontracted host method should lower");
    let function = only_function(&program);
    let call = function
        .statements()
        .find_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Host(MirHostOperation::Call { .. }) => Some(statement),
            _ => None,
        })
        .expect("host method call");
    let Some(MirPlace::Temp(destination)) = call.destination else {
        panic!(
            "host call destination is not a temp: {:?}",
            call.destination
        )
    };
    assert_eq!(
        function
            .temp(destination)
            .expect("uncontracted call temp")
            .value_type,
        MirValueType::Dynamic,
        "{}",
        program.dump()
    );
}

#[test]
fn mir_builder_preserves_precise_host_boundary_definition_and_unit_contracts() {
    let source = r#"
fn main(host) {
    let record = host.record;
    let state = host.state;
    return host.nothing;
}
"#;
    let program = build_host(source).expect("typed host reads should lower");
    let function = only_function(&program);
    let read_types = function
        .statements()
        .filter_map(
            |(_, statement)| match (&statement.kind, statement.destination) {
                (
                    MirStatementKind::Host(MirHostOperation::Read { .. }),
                    Some(MirPlace::Temp(temp)),
                ) => Some(function.temp(temp).expect("host read temp").value_type),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(
        read_types,
        vec![
            MirValueType::ScriptType {
                type_id: SCRIPT_RECORD_TYPE_ID,
                shape: SCRIPT_RECORD_SHAPE,
            },
            MirValueType::Enum(SCRIPT_ENUM_TYPE_ID),
            MirValueType::Unit,
        ],
        "{}",
        program.dump()
    );
}

#[test]
fn mir_builder_evaluates_host_root_nested_read_key_and_arguments_once_in_order() {
    let source = r#"
fn main(host, lookup, key, rhs) {
    return host.items[lookup.items[key].amount].grant(rhs);
}
"#;
    let program = build_host(source).expect("nested host key should lower");
    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let host_positions = statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            matches!(statement.kind, MirStatementKind::Host(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(host_positions.len(), 2, "{}", program.dump());
    let nested_read = host_positions[0];
    let outer_call = host_positions[1];
    assert!(nested_read < outer_call);

    let MirStatementKind::Host(MirHostOperation::Read {
        root: nested_root,
        path: nested_path,
    }) = &statements[nested_read].kind
    else {
        panic!("first host boundary is not the nested read")
    };
    let MirStatementKind::Host(MirHostOperation::Call {
        root: outer_root,
        path: outer_path,
        arguments,
        ..
    }) = &statements[outer_call].kind
    else {
        panic!("second host boundary is not the outer call")
    };
    assert!(matches!(nested_root, MirOperand::Temp(_)));
    assert!(matches!(outer_root, MirOperand::Temp(_)));
    assert_eq!(nested_path.segments.len(), 3);
    assert_eq!(outer_path.segments.len(), 2);
    let nested_key = match &nested_path.segments[1] {
        MirHostPathSegment::Index { value, .. } => value,
        segment => panic!("nested path lost its dynamic index: {segment:?}"),
    };
    let outer_key = match &outer_path.segments[1] {
        MirHostPathSegment::Index { value, .. } => value,
        segment => panic!("outer path lost its dynamic index: {segment:?}"),
    };
    let nested_result = match statements[nested_read].destination {
        Some(MirPlace::Temp(temp)) => MirOperand::Temp(temp),
        destination => panic!("nested host read destination: {destination:?}"),
    };
    assert!(matches!(nested_key, MirOperand::Temp(_)));
    assert_eq!(outer_key, &nested_result);
    assert_eq!(arguments.len(), 1);
    assert!(matches!(arguments[0], MirOperand::Temp(_)));

    let outer_root_definition = operand_definition_position(function, outer_root)
        .expect("captured outer host root definition");
    let nested_root_definition = operand_definition_position(function, nested_root)
        .expect("captured nested host root definition");
    let argument_definition = operand_definition_position(function, &arguments[0])
        .expect("captured host argument definition");
    assert!(
        outer_root_definition < nested_root_definition
            && nested_root_definition < nested_read
            && nested_read < argument_definition
            && argument_definition < outer_call,
        "{}",
        program.dump()
    );
}

#[test]
fn mir_builder_prepares_a_host_assignment_target_before_evaluating_its_rhs() {
    let source = r#"
fn main(host, lookup, key, rhs) {
    host.items[key].amount = lookup.items[rhs].amount;
}
"#;
    let program = build_host(source).expect("effectful host assignment should lower");
    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let host_positions = statements
        .iter()
        .enumerate()
        .filter_map(|(position, statement)| {
            matches!(statement.kind, MirStatementKind::Host(_)).then_some(position)
        })
        .collect::<Vec<_>>();
    let [read_position, write_position] = host_positions.as_slice() else {
        panic!(
            "expected one RHS read and one target write: {}",
            program.dump()
        )
    };
    let MirStatementKind::Host(MirHostOperation::Read {
        root: rhs_root,
        path: rhs_path,
    }) = &statements[*read_position].kind
    else {
        panic!("first host boundary is not the RHS read")
    };
    let MirStatementKind::Host(MirHostOperation::Write {
        root: target_root,
        path: target_path,
        value,
    }) = &statements[*write_position].kind
    else {
        panic!("second host boundary is not the target write")
    };
    let target_key = dynamic_index_operand(target_path);
    let rhs_key = dynamic_index_operand(rhs_path);
    assert_eq!(
        value,
        &statements[*read_position]
            .destination
            .map(|destination| match destination {
                MirPlace::Temp(temp) => MirOperand::Temp(temp),
                MirPlace::Local(local) => MirOperand::Local(local),
            })
            .expect("RHS read destination")
    );
    let target_root_definition = operand_definition_position(function, target_root)
        .expect("captured assignment target root");
    let target_key_definition =
        operand_definition_position(function, target_key).expect("captured assignment target key");
    let rhs_root_definition =
        operand_definition_position(function, rhs_root).expect("captured RHS root");
    let rhs_key_definition =
        operand_definition_position(function, rhs_key).expect("captured RHS key");
    assert!(
        target_root_definition < target_key_definition
            && target_key_definition < rhs_root_definition
            && rhs_root_definition < rhs_key_definition
            && rhs_key_definition < *read_position
            && read_position < write_position,
        "{}",
        program.dump()
    );
    assert_eq!(
        defined_function_dump(&program),
        r#"  fn f0 body h0 owner function#9100 symbol="host_builder::main" @91:33..91/h0 {
    param p0: host -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @91:9..13/h0
    param p1: lookup -> l1 kind=Explicit(HirParamId(1)) contract=None default=None hir=l1 @91:15..21/h0
    param p2: key -> l2 kind=Explicit(HirParamId(2)) contract=None default=None hir=l2 @91:23..26/h0
    param p3: rhs -> l3 kind=Explicit(HirParamId(3)) contract=None default=None hir=l3 @91:28..31/h0
    local l0: Script(HirLocalId(0)) Dynamic @91:9..13/h0
    local l1: Script(HirLocalId(1)) Dynamic @91:15..21/h0
    local l2: Script(HirLocalId(2)) Dynamic @91:23..26/h0
    local l3: Script(HirLocalId(3)) Dynamic @91:28..31/h0
    temp t0: Dynamic def=s0 @91:39..43/e4
    temp t1: Dynamic def=s1 @91:50..53/e5
    temp t2: Dynamic def=s2 @91:64..70/e9
    temp t3: Dynamic def=s3 @91:77..80/e10
    temp t4: Primitive(I64) def=s4 @91:64..88/e6
    debug dl0: host -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @91:9..13/h0
    debug dl1: lookup -> l1 kind=Parameter hir=Some(1) scope=h0 live=[] @91:15..21/h0
    debug dl2: key -> l2 kind=Parameter hir=Some(2) scope=h0 live=[] @91:23..26/h0
    debug dl3: rhs -> l3 kind=Parameter hir=Some(3) scope=h0 live=[] @91:28..31/h0
    safepoint sp0: live={} @91:64..88/e6
    bb0:
      s0: t0 = l0 [pure] @91:39..43/e4
      s1: t1 = l2 [pure] @91:50..53/e5
      s2: t2 = l1 [pure] @91:64..70/e9
      s3: t3 = l3 [pure] @91:77..80/e10
      s4: t4 = host.read t2 type#9102 path=[Field(HostFieldTarget { owner: HostTypeTarget { semantic: TypeId(9101), runtime: HostTypeId(9102) }, semantic: FieldId(9107), runtime: FieldId(9107), access: CompileFieldAccess { readable: true, writable: true, reflect_readable: true, reflect_writable: true, required_permissions: ["state.read", "state.write"] } }), Index { value: Temp(MirTempId(3)), capability: CompileHostIndexCapability { readable: true, writable: true, mutable: true, removable: true, key: Some(Primitive(I64)), value: Some(Host(HostTypeTarget { semantic: TypeId(9105), runtime: HostTypeId(9106) })) } }, Field(HostFieldTarget { owner: HostTypeTarget { semantic: TypeId(9105), runtime: HostTypeId(9106) }, semantic: FieldId(9108), runtime: FieldId(9108), access: CompileFieldAccess { readable: true, writable: true, reflect_readable: true, reflect_writable: true, required_permissions: ["state.read", "state.write"] } })] [trap|alloc|host-read, sp0] @91:64..88/e6
      s5: host.write t0 type#9102 path=[Field(HostFieldTarget { owner: HostTypeTarget { semantic: TypeId(9101), runtime: HostTypeId(9102) }, semantic: FieldId(9107), runtime: FieldId(9107), access: CompileFieldAccess { readable: true, writable: true, reflect_readable: true, reflect_writable: true, required_permissions: ["state.read", "state.write"] } }), Index { value: Temp(MirTempId(1)), capability: CompileHostIndexCapability { readable: true, writable: true, mutable: true, removable: true, key: Some(Primitive(I64)), value: Some(Host(HostTypeTarget { semantic: TypeId(9105), runtime: HostTypeId(9106) })) } }, Field(HostFieldTarget { owner: HostTypeTarget { semantic: TypeId(9105), runtime: HostTypeId(9106) }, semantic: FieldId(9108), runtime: FieldId(9108), access: CompileFieldAccess { readable: true, writable: true, reflect_readable: true, reflect_writable: true, required_permissions: ["state.read", "state.write"] } })], t4 [trap|host-read|host-write] @91:39..88/e0
      -> return unit [pure] @91:33..91/h0
  }
}
"#
    );
}

#[test]
fn mir_builder_stops_host_path_lowering_when_a_dynamic_key_diverges() {
    let source = r#"
fn main(host, key) {
    host.items[{ return key; }].amount = "later";
}
"#;
    let program = build_host(source).expect("diverging host key should finish the function");
    let dump = program.dump();
    assert!(dump.contains("-> return l1"), "{dump}");
    assert!(!dump.contains("host.write"), "{dump}");
    assert!(!dump.contains("const.materialize \"later\""), "{dump}");
}

fn operand_definition_position(
    function: &crate::MirFunction,
    operand: &MirOperand,
) -> Option<usize> {
    let MirOperand::Temp(temp) = operand else {
        return None;
    };
    let definition = function.temp(*temp)?.definition()?;
    function
        .statements()
        .position(|(statement, _)| statement == definition)
}

fn dynamic_index_operand(path: &crate::MirHostPath) -> &MirOperand {
    path.segments
        .iter()
        .find_map(|segment| match segment {
            MirHostPathSegment::Index { value, .. } => Some(value),
            _ => None,
        })
        .expect("dynamic host index")
}

fn defined_function_dump(program: &MirProgram) -> String {
    let dump = program.dump();
    let start = dump.find("  fn ").expect("defined MIR function");
    dump[start..].to_owned()
}

fn only_function(program: &MirProgram) -> &crate::MirFunction {
    let functions = program
        .functions()
        .map(|(_, function)| function)
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!("expected one MIR function, got {}", functions.len())
    };
    function
}

fn expression_origin(body: &HirBody, expression: HirExprId) -> MirSourceOrigin {
    let expression = body.expression(expression).expect("fixture expression");
    MirSourceOrigin::expression(body.id, expression.id, expression.origin.span)
}

fn assignment_operator_target(source: &str) -> OperatorTargetFact {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(92),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("host_operator"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id)],
    )
    .expect("operator fixture analysis");
    let assignment = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Assign { .. }))
        .expect("host assignment");
    analysis
        .view(ROOT_FUNCTION)
        .expect("root analysis")
        .operator_target(assignment.id)
        .expect("assignment operator target")
}
