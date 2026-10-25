use vela_common::PrimitiveTag;
use vela_def::{FieldId, FunctionId, MethodId, TypeId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirNodeId};

use crate::*;

#[test]
fn mir_guard_context_keeps_parameter_indices_backend_neutral() {
    let index = u32::from(u16::MAX) + 1;
    let guard = CompileGuardTarget::new(
        MirTypeContract::Primitive(PrimitiveTag::I64),
        MirGuardLocation::Parameter { index },
        "value",
    );

    assert_eq!(
        guard.context,
        MirGuardContext::new(MirGuardLocation::Parameter { index }, "value")
    );
}

#[test]
fn compile_targets_retain_opaque_external_dispatch_owners() {
    let origin = MirSourceOrigin::declaration(
        HirDeclId::new(299),
        vela_common::Span::new(vela_common::SourceId::new(6), 1, 9),
    );
    let owner = TypeId::new(298);
    let mut snapshot = CompileTargetSnapshot::builder();
    snapshot
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: owner,
                canonical_name: "Player".to_owned(),
                runtime_name: "Player".to_owned(),
                class: CompileTypeClass::OpaqueExternal,
                shape: None,
                fields: Vec::new(),
                variants: Vec::new(),
            },
            origin,
        )
        .expect("opaque external owner descriptor should be retained");

    assert_eq!(
        snapshot
            .build()
            .expect("opaque schema-only snapshot should validate")
            .type_descriptor(owner)
            .map(|ty| ty.class),
        Some(CompileTypeClass::OpaqueExternal)
    );
}

#[test]
fn mir_model_compile_targets_select_behavior_intrinsics_without_names() {
    let origin = MirSourceOrigin::body(
        HirBodyId::new(300),
        vela_common::Span::new(vela_common::SourceId::new(7), 0, 5),
    );
    let function = FunctionId::new(299);
    let reflection = CompileCallTarget::positional(
        CompileCalleeTarget::Reflection {
            operation: CompileReflectionCall::Call,
            function: FunctionId::new(301),
            debug_name: "reflect::call".to_owned(),
        },
        vec![HirExprId::new(311), HirExprId::new(312)],
    );
    let set = CompileCallTarget::positional(
        CompileCalleeTarget::SetFromArray {
            function: FunctionId::new(302),
            debug_name: "set::from_array".to_owned(),
        },
        vec![HirExprId::new(313)],
    );
    let script = CompileCallTarget::script(
        CompileCalleeTarget::ScriptFunction {
            function: FunctionId::new(315),
            debug_name: "game::defaulted".to_owned(),
        },
        vec![HirExprId::new(316)],
        vec![
            CompilePlacedCallArgument::placed(0, 0, HirExprId::new(316)),
            CompilePlacedCallArgument::missing(1),
        ],
    );
    let dynamic = CompileCallTarget::dynamic(
        CompileCalleeTarget::DynamicMethod(DynamicMethodTarget::method(
            "invoke",
            0,
            vec!["value".to_owned()],
        )),
        vec![CompileDynamicCallArgument {
            name: Some("value".to_owned()),
            value: HirExprId::new(317),
        }],
    );
    let expressions = [
        (HirExprId::new(307), reflection.clone()),
        (HirExprId::new(308), set.clone()),
        (HirExprId::new(318), script.clone()),
        (HirExprId::new(319), dynamic.clone()),
    ];
    let mut snapshot = CompileTargetSnapshot::builder();
    for (expression, target) in &expressions {
        snapshot
            .insert_call(function, *expression, target.clone(), origin)
            .expect("intrinsic call target should be unique");
    }
    let snapshot = snapshot.build_unchecked();

    assert_eq!(snapshot.call(function, expressions[0].0), Some(&reflection));
    assert_eq!(snapshot.call(function, expressions[1].0), Some(&set));
    assert_eq!(snapshot.call(function, expressions[2].0), Some(&script));
    assert_eq!(snapshot.call(function, expressions[3].0), Some(&dynamic));
}

#[test]
fn executable_placements_are_scoped_by_stable_function_identity() {
    let expression = HirExprId::new(370);
    let first_function = FunctionId::new(371);
    let second_function = FunctionId::new(372);
    let missing_function = FunctionId::new(373);
    let origin = MirSourceOrigin::expression(
        HirBodyId::new(374),
        expression,
        vela_common::Span::new(vela_common::SourceId::new(12), 4, 16),
    );
    let first_call = CompileCallTarget::dynamic(
        CompileCalleeTarget::DynamicCallable,
        vec![CompileDynamicCallArgument {
            name: Some("left".to_owned()),
            value: HirExprId::new(375),
        }],
    );
    let second_call = CompileCallTarget::dynamic(
        CompileCalleeTarget::DynamicCallable,
        vec![CompileDynamicCallArgument {
            name: Some("right".to_owned()),
            value: HirExprId::new(376),
        }],
    );
    let first_member = CompileMemberTarget::Dynamic {
        name: "first".to_owned(),
    };
    let second_member = CompileMemberTarget::Dynamic {
        name: "second".to_owned(),
    };
    let first_guard = CompileGuardTarget::new(
        MirTypeContract::Primitive(PrimitiveTag::I64),
        MirGuardLocation::Local,
        "first_value",
    );
    let second_guard = CompileGuardTarget::new(
        MirTypeContract::Primitive(PrimitiveTag::String),
        MirGuardLocation::Field,
        "second_value",
    );
    let mut snapshot = CompileTargetSnapshot::builder();
    snapshot
        .insert_call(first_function, expression, first_call.clone(), origin)
        .expect("the first function owns its call placement");
    snapshot
        .insert_call(second_function, expression, second_call.clone(), origin)
        .expect("a second function may reuse the same HIR expression identity");
    snapshot
        .insert_member(first_function, expression, first_member.clone(), origin)
        .expect("the first function owns its member placement");
    snapshot
        .insert_member(second_function, expression, second_member.clone(), origin)
        .expect("member placements use the same executable scope as calls");
    snapshot
        .insert_guard(
            CompileGuardKey::Expression {
                function: first_function,
                expression,
            },
            first_guard.clone(),
            origin,
        )
        .expect("the first expression guard should be unique in its function");
    snapshot
        .insert_guard(
            CompileGuardKey::Expression {
                function: second_function,
                expression,
            },
            second_guard.clone(),
            origin,
        )
        .expect("the second expression guard should not collide across functions");
    let snapshot = snapshot.build_unchecked();

    assert_eq!(snapshot.call(first_function, expression), Some(&first_call));
    assert_eq!(
        snapshot.call(second_function, expression),
        Some(&second_call)
    );
    assert_eq!(
        snapshot.member(first_function, expression),
        Some(&first_member)
    );
    assert_eq!(
        snapshot.member(second_function, expression),
        Some(&second_member)
    );
    assert_eq!(
        snapshot.guard(CompileGuardKey::Expression {
            function: first_function,
            expression,
        }),
        Some(&first_guard)
    );
    assert_eq!(
        snapshot.guard(CompileGuardKey::Expression {
            function: second_function,
            expression,
        }),
        Some(&second_guard)
    );
    assert_eq!(snapshot.call(missing_function, expression), None);
    assert_eq!(snapshot.member(missing_function, expression), None);
    assert_eq!(
        snapshot.guard(CompileGuardKey::Expression {
            function: missing_function,
            expression,
        }),
        None
    );
}

#[test]
fn mir_model_compile_snapshot_owns_source_identity_and_diagnostic_origins() {
    let body = HirBodyId::new(320);
    let origin = MirSourceOrigin::body(
        body,
        vela_common::Span::new(vela_common::SourceId::new(8), 3, 12),
    );
    let function_declaration = HirDeclId::new(321);
    let function = FunctionId::new(322);
    let type_declaration = HirDeclId::new(323);
    let type_id = TypeId::new(324);
    let method = MethodExecutableTarget {
        method: MethodId::new(325),
        function: FunctionId::new(326),
        owner: type_id,
        node: HirNodeId::new(327),
    };
    let mut snapshot = CompileTargetSnapshot::builder();
    snapshot
        .insert_script_function(
            function_declaration,
            body,
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "game::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters: vec![CompileParameter {
                        name: "value".to_owned(),
                        contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                        default: CompileParameterDefault::Required,
                        origin: Some(origin),
                    }],
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(true),
            },
            origin,
        )
        .expect("script function identity should be inserted atomically");
    snapshot
        .insert_script_method(body, method, origin)
        .expect("method node should map to its owner-qualified executable");
    snapshot
        .insert_script_type(
            type_declaration,
            CompileTypeDescriptor {
                id: type_id,
                canonical_name: "game::Player".to_owned(),
                runtime_name: "game::Player".to_owned(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(vela_common::ShapeId::new(328)),
                fields: Vec::new(),
                variants: Vec::new(),
            },
            origin,
        )
        .expect("script type identity should be inserted atomically");
    let referenced_declaration = HirDeclId::new(329);
    let referenced_function = FunctionId::new(330);
    snapshot
        .insert_script_function_descriptor(
            referenced_declaration,
            CompileFunctionDescriptor {
                id: referenced_function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "game::helper".to_owned(),
                debug_name: "helper".to_owned(),
                signature: CompileSignature {
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(true),
            },
            origin,
        )
        .expect("non-root script functions still need stable call descriptors");
    let referenced_method = MethodExecutableTarget {
        method: MethodId::new(331),
        function: FunctionId::new(332),
        owner: type_id,
        node: HirNodeId::new(333),
    };
    snapshot
        .insert_script_method_target(referenced_method, origin)
        .expect("non-root methods still need stable call targets");
    let shared_default_method = MethodExecutableTarget {
        method: method.method,
        function: FunctionId::new(334),
        owner: TypeId::new(335),
        node: method.node,
    };
    snapshot
        .insert_script_method_target(shared_default_method, origin)
        .expect("one trait default node may instantiate for another receiver owner");
    let snapshot = snapshot.build_unchecked();

    assert_eq!(
        snapshot.function_for_declaration(function_declaration),
        Some(function)
    );
    assert_eq!(
        snapshot.method_for_node(method.node, method.owner),
        Some(method)
    );
    assert_eq!(
        snapshot.function_for_declaration(referenced_declaration),
        Some(referenced_function)
    );
    assert_eq!(
        snapshot.method_for_node(referenced_method.node, referenced_method.owner),
        Some(referenced_method)
    );
    assert_eq!(
        snapshot.method_for_node(method.node, shared_default_method.owner),
        Some(shared_default_method)
    );
    assert_eq!(snapshot.methods_for_node(method.node).len(), 2);
    assert_eq!(
        snapshot.type_for_declaration(type_declaration),
        Some(type_id)
    );
    assert_eq!(
        snapshot
            .type_by_name("game::Player")
            .map(|descriptor| descriptor.id),
        Some(type_id)
    );
    assert_eq!(
        snapshot
            .function_descriptor(function)
            .and_then(|descriptor| descriptor.signature.parameters[0].origin),
        Some(origin)
    );
    assert_eq!(
        snapshot
            .compilation_roots()
            .map(|(function, _)| function)
            .collect::<Vec<_>>(),
        [function, method.function]
    );
}

#[test]
fn mir_input_callable_contracts_keep_kind_and_optional_arity_independent() {
    let function = MirTypeContract::Callable {
        accepted_kinds: crate::MirCallableKindSet::FUNCTION,
        positional_arity: None,
    };
    let zero_arg_function = MirTypeContract::Callable {
        accepted_kinds: crate::MirCallableKindSet::FUNCTION,
        positional_arity: Some(0),
    };
    let closure = MirTypeContract::Callable {
        accepted_kinds: crate::MirCallableKindSet::CLOSURE,
        positional_arity: None,
    };

    assert_ne!(function, zero_arg_function);
    assert_ne!(function, closure);
    assert!(matches!(
        function,
        MirTypeContract::Callable {
            accepted_kinds: crate::MirCallableKindSet::FUNCTION,
            positional_arity: None,
        }
    ));
    assert!(matches!(
        closure,
        MirTypeContract::Callable {
            accepted_kinds: crate::MirCallableKindSet::CLOSURE,
            positional_arity: None,
        }
    ));
}

#[test]
fn mir_model_target_access_is_complete_and_registry_independent() {
    let origin = MirSourceOrigin::body(
        HirBodyId::new(340),
        vela_common::Span::new(vela_common::SourceId::new(9), 4, 18),
    );
    let function = FunctionId::new(341);
    let owner = TypeId::new(342);
    let method = MethodId::new(343);
    let field = FieldId::new(344);
    let function_access = CompileFunctionAccess::new(false, true, true);
    let method_access = CompileMethodAccess::new(
        false,
        false,
        vec![
            "player.reward".to_owned(),
            "player.admin".to_owned(),
            "player.reward".to_owned(),
        ],
    );
    let field_access =
        CompileFieldAccess::new(true, false, true, false, vec!["player.inspect".to_owned()]);
    let signature = CompileSignature {
        parameters: Vec::new(),
        positional: CompilePositionalPolicy::RuntimeChecked,
        return_contract: None,
        effect: MirEffect::may_trap(),
    };
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_function_descriptor(
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Registry,
                canonical_symbol: "admin::rewrite".to_owned(),
                debug_name: "admin::rewrite".to_owned(),
                signature: signature.clone(),
                access: function_access,
            },
            origin,
        )
        .expect("function access should be copied into its target descriptor");
    targets
        .insert_method_descriptor(
            CompileMethodDescriptor {
                id: method,
                owner,
                member_name: "reward".to_owned(),
                debug_name: "Player::reward".to_owned(),
                class: CompileMethodClass::Registry,
                signature: signature.clone(),
                access: method_access.clone(),
            },
            origin,
        )
        .expect("method access should be copied into its target descriptor");
    targets
        .insert_field_descriptor(
            CompileFieldDescriptor {
                id: field,
                owner,
                variant: None,
                name: "secret".to_owned(),
                contract: None,
                declaration_order: 0,
                access: field_access.clone(),
            },
            origin,
        )
        .expect("field access should be copied into its target descriptor");
    let targets = targets.build_unchecked();

    assert_eq!(
        targets
            .function_descriptor(function)
            .map(|descriptor| descriptor.access),
        Some(function_access)
    );
    assert_eq!(
        targets
            .method_descriptor(owner, method)
            .map(|descriptor| &descriptor.access),
        Some(&method_access)
    );
    assert_eq!(
        targets
            .field_descriptor(field)
            .map(|descriptor| &descriptor.access),
        Some(&field_access)
    );
    assert_eq!(
        method_access.required_permissions(),
        ["player.admin", "player.reward"]
    );
    assert_eq!(field_access.required_permissions(), ["player.inspect"]);

    assert_eq!(
        CompileFunctionAccess::script(false),
        CompileFunctionAccess::new(false, true, false)
    );
    assert_eq!(
        CompileMethodAccess::script(),
        CompileMethodAccess::new(true, true, Vec::new())
    );
    assert_eq!(
        CompileFieldAccess::script(),
        CompileFieldAccess::new(true, true, true, true, Vec::new())
    );

    let dump = MirProgram::new(targets.target_table().clone()).dump();
    assert!(dump.contains("reflect_visible: true"));
    assert!(dump.contains("required_permissions: [\"player.admin\", \"player.reward\"]"));
    assert!(dump.contains("required_permissions: [\"player.inspect\"]"));
}
