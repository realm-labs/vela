use vela_common::{PrimitiveTag, SourceId, Span};
use vela_def::FunctionId;
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirLocalId, HirParamId};

use crate::*;

fn origin(body: HirBodyId, seed: u32) -> MirSourceOrigin {
    MirSourceOrigin::body(
        body,
        Span::new(SourceId::new(61), seed, seed.saturating_add(5)),
    )
}

fn insert_root(builder: &mut CompileTargetSnapshotBuilder, function: FunctionId, body: HirBodyId) {
    builder
        .insert_script_function(
            HirDeclId::new(1),
            body,
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "test::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    asyncness: vela_common::CallableAsyncness::Sync,
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(false),
            },
            origin(body, 1),
        )
        .expect("root insertion");
}

fn lambda(body: HirBodyId, parent: HirBodyId, contract: MirTypeContract) -> CompileLambdaTarget {
    CompileLambdaTarget {
        body,
        parent,
        expression: HirExprId::new(3),
        code_symbol: "test::main::<lambda@10>".to_owned(),
        parameters: vec![CompileLambdaParameterTarget {
            parameter: HirParamId::new(4),
            local: HirLocalId::new(5),
            name: "value".to_owned(),
            contract: Some(contract),
            origin: origin(body, 11),
        }],
        origin: origin(body, 10),
    }
}

#[test]
fn snapshot_exposes_generation_local_lambda_parameter_contracts() {
    let function = FunctionId::new(600);
    let root = HirBodyId::new(1);
    let body = HirBodyId::new(2);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(&mut builder, function, root);
    builder
        .insert_lambda(
            function,
            lambda(body, root, MirTypeContract::Primitive(PrimitiveTag::I64)),
        )
        .expect("lambda insertion");

    let snapshot = builder.build().expect("closed lambda snapshot");
    let scoped = snapshot.function_targets(function).expect("root targets");
    let target = scoped.lambda(body).expect("lambda target");
    assert_eq!(target.parent, root);
    assert_eq!(
        scoped
            .lambda_parameter(body, HirParamId::new(4))
            .and_then(|parameter| parameter.contract.as_ref()),
        Some(&MirTypeContract::Primitive(PrimitiveTag::I64))
    );
    assert_eq!(snapshot.compilation_roots().count(), 1);
    assert!(snapshot.functions_for_body(body).is_empty());
}

#[test]
fn snapshot_rejects_lambda_parent_and_contract_edges_outside_the_root() {
    let function = FunctionId::new(610);
    let root = HirBodyId::new(11);
    let body = HirBodyId::new(12);
    let mut missing_parent = CompileTargetSnapshot::builder();
    insert_root(&mut missing_parent, function, root);
    missing_parent
        .insert_lambda(
            function,
            lambda(
                body,
                HirBodyId::new(99),
                MirTypeContract::Primitive(PrimitiveTag::Bool),
            ),
        )
        .expect("lambda insertion");
    assert!(
        missing_parent
            .build()
            .expect_err("missing parent must fail")
            .to_string()
            .contains("neither the root nor another lambda")
    );

    let mut missing_contract = CompileTargetSnapshot::builder();
    insert_root(&mut missing_contract, function, root);
    missing_contract
        .insert_lambda(
            function,
            lambda(
                body,
                root,
                MirTypeContract::Definition(vela_def::TypeId::new(999)),
            ),
        )
        .expect("lambda insertion");
    assert!(
        missing_contract
            .build()
            .expect_err("missing contract descriptor must fail")
            .to_string()
            .contains("missing type #999")
    );
}
