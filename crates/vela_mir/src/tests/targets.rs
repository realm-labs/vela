use vela_common::{HostTypeId, PrimitiveTag};
use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirNodeId};

use crate::*;

#[test]
fn mir_model_compile_targets_select_behavior_intrinsics_without_names() {
    let origin = MirSourceOrigin::body(
        HirBodyId::new(300),
        vela_common::Span::new(vela_common::SourceId::new(7), 0, 5),
    );
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
    let path = CompileHostPathTarget {
        root: HirExprId::new(303),
        root_type: HostTypeTarget {
            semantic: TypeId::new(304),
            runtime: HostTypeId::new(305),
        },
        segments: vec![CompileHostPathSegment::DynamicIndex {
            expression: HirExprId::new(306),
            capability: CompileHostIndexCapability {
                readable: true,
                writable: true,
                mutable: true,
                removable: true,
                key: None,
                value: None,
            },
        }],
    };
    let remove = CompileCallTarget::positional(
        CompileCalleeTarget::HostRemove { path: path.clone() },
        Vec::new(),
    );
    let push = CompileCallTarget::positional(
        CompileCalleeTarget::HostPush { path },
        vec![HirExprId::new(314)],
    );
    let script = CompileCallTarget::script(
        CompileCalleeTarget::ScriptFunction {
            function: FunctionId::new(315),
            debug_name: "game::defaulted".to_owned(),
        },
        vec![
            CompileScriptCallArgument {
                parameter: 0,
                value: Some(HirExprId::new(316)),
            },
            CompileScriptCallArgument {
                parameter: 1,
                value: None,
            },
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
        (HirExprId::new(309), remove.clone()),
        (HirExprId::new(310), push.clone()),
        (HirExprId::new(318), script.clone()),
        (HirExprId::new(319), dynamic.clone()),
    ];
    let mut snapshot = CompileTargetSnapshot::builder();
    for (expression, target) in &expressions {
        snapshot
            .insert_call(*expression, target.clone(), origin)
            .expect("intrinsic call target should be unique");
    }
    let snapshot = snapshot.build();

    assert_eq!(snapshot.call(expressions[0].0), Some(&reflection));
    assert_eq!(snapshot.call(expressions[1].0), Some(&set));
    assert_eq!(snapshot.call(expressions[2].0), Some(&remove));
    assert_eq!(snapshot.call(expressions[3].0), Some(&push));
    assert_eq!(snapshot.call(expressions[4].0), Some(&script));
    assert_eq!(snapshot.call(expressions[5].0), Some(&dynamic));
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
    let snapshot = snapshot.build();

    assert_eq!(
        snapshot.function_for_declaration(function_declaration),
        Some(function)
    );
    assert_eq!(snapshot.method_for_node(method.node), Some(method));
    assert_eq!(
        snapshot.function_for_declaration(referenced_declaration),
        Some(referenced_function)
    );
    assert_eq!(
        snapshot.method_for_node(referenced_method.node),
        Some(referenced_method)
    );
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
