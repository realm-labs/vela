use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{HostTypeId, PrimitiveTag, ScalarValue, SourceId};
use vela_def::{FieldId, FunctionId, GlobalId, TypeId};
use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirBody, HirExprKind};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileFunctionIdentity, CompileGlobalDescriptor, CompileGuardKey,
    CompileGuardTarget, CompileHostPathSegment, CompileHostPathTarget, CompileMemberTarget,
    CompilePositionalPolicy, CompileSignature, CompileTargetSnapshot, CompileTargetSnapshotBuilder,
    CompileTypeClass, CompileTypeDescriptor, HostFieldTarget, HostTypeTarget, MirBuildError,
    MirEffect, MirEvaluatedConstant, MirGlobalOperation, MirGuardLocation, MirHostOperation,
    MirLoweringConfig, MirLoweringInput, MirOperand, MirPlace, MirRvalue, MirSourceOrigin,
    MirStatementKind, MirTypeContract, MirValueType,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(9_900);
const GLOBAL: GlobalId = GlobalId::new(9_901);
const HOST_TYPE_ID: TypeId = TypeId::new(9_902);
const HOST_RUNTIME_ID: HostTypeId = HostTypeId::new(9_903);
const HOST_FIELD_ID: FieldId = FieldId::new(9_904);
const HOST_TYPE: HostTypeTarget = HostTypeTarget {
    semantic: HOST_TYPE_ID,
    runtime: HOST_RUNTIME_ID,
};

fn try_build_declarations(
    source: &str,
    configure: impl FnOnce(
        &ModuleGraph,
        &HirBody,
        &mut CompileTargetSnapshotBuilder,
    ) -> Result<(), MirBuildError>,
) -> Result<crate::MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(99),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("declarations"),
        source,
    ));
    try_build_declaration_graph(graph, configure)
}

fn try_build_declaration_graph(
    mut graph: ModuleGraph,
    configure: impl FnOnce(
        &ModuleGraph,
        &HirBody,
        &mut CompileTargetSnapshotBuilder,
    ) -> Result<(), MirBuildError>,
) -> Result<crate::MirProgram, MirBuildError> {
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = declaration(&graph, "main");
    let body = graph.function_body(main).expect("main body");
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id)],
    )
    .expect("declaration analysis");
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        main,
        body.id,
        CompileFunctionDescriptor {
            id: ROOT_FUNCTION,
            class: CompileFunctionClass::Script,
            canonical_symbol: "declarations::main".to_owned(),
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
        origin,
    )?;
    configure(&graph, body, &mut targets)?;
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(ROOT_FUNCTION),
        body.id,
        analysis.view(ROOT_FUNCTION).expect("main analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;
    crate::build_mir(input)
}

fn declaration(graph: &ModuleGraph, name: &str) -> HirDeclId {
    graph
        .declarations()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("declaration {name:?}"))
        .id
}

fn insert_global(
    graph: &ModuleGraph,
    targets: &mut CompileTargetSnapshotBuilder,
    declaration: HirDeclId,
    contract: MirTypeContract,
) -> Result<(), MirBuildError> {
    let metadata = graph.declaration(declaration).expect("global declaration");
    let origin = MirSourceOrigin::declaration(declaration, metadata.span);
    targets.insert_global(
        declaration,
        CompileGlobalDescriptor {
            id: GLOBAL,
            name: format!("declarations::{}", metadata.name),
            contract: contract.clone(),
        },
        origin,
    )?;
    if contract != MirTypeContract::Any {
        targets.insert_guard(
            CompileGuardKey::Global(declaration),
            CompileGuardTarget::new(contract, MirGuardLocation::Global, metadata.name.clone()),
            origin,
        )?;
    }
    Ok(())
}

fn insert_constant(
    graph: &ModuleGraph,
    targets: &mut CompileTargetSnapshotBuilder,
    declaration: HirDeclId,
    value: MirEvaluatedConstant,
) -> Result<(), MirBuildError> {
    let body = graph
        .const_initializer_body(declaration)
        .expect("const initializer body");
    targets.insert_evaluated_constant(
        declaration,
        value,
        MirSourceOrigin::body(body.id, body.origin.span),
    )
}

#[test]
fn declaration_paths_distinguish_globals_and_scalar_constants_without_read_guards() {
    let source = r#"
global state: i64
const STEP = 3
fn main() {
    let first = state;
    let one = STEP;
    let two = STEP;
    let second = state;
    return two;
}
"#;
    let program = try_build_declarations(source, |graph, _body, targets| {
        insert_global(
            graph,
            targets,
            declaration(graph, "state"),
            MirTypeContract::Primitive(PrimitiveTag::I64),
        )?;
        insert_constant(
            graph,
            targets,
            declaration(graph, "STEP"),
            MirEvaluatedConstant::Scalar(ScalarValue::I64(3)),
        )
    })
    .expect("global and scalar const paths");
    let (_, function) = program.functions().next().expect("main function");
    let global_reads = function
        .statements()
        .filter_map(|(_, statement)| match statement.kind {
            MirStatementKind::Global(MirGlobalOperation::Read { global }) => {
                Some((statement, global))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(global_reads.len(), 2, "{}", program.dump());
    for (statement, global) in global_reads {
        assert_eq!(global, GLOBAL);
        assert_eq!(statement.effect, MirEffect::global_read());
        assert_eq!(statement.safepoint, None);
        assert_eq!(source_text(source, statement.origin), "state");
        let Some(MirPlace::Temp(temp)) = statement.destination else {
            panic!("global read needs a temp destination")
        };
        assert_eq!(
            function.temp(temp).expect("global temp").value_type,
            MirValueType::Primitive(PrimitiveTag::I64)
        );
    }
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::Assign(MirRvalue::Constant {
                    value: crate::MirImmediate::Scalar(ScalarValue::I64(3)),
                    provenance: crate::MirConstantProvenance::EvaluatedConstant,
                })
            ))
            .count(),
        2,
        "{}",
        program.dump()
    );
    assert!(!function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::MaterializeConstant(_) | MirStatementKind::GuardTrap { .. }
    )));
    assert_eq!(function.guards().count(), 0);
}

#[test]
fn heap_constants_materialize_at_every_use_in_source_order_and_at_use_origins() {
    let source = r#"
const LABEL = "compiled"
fn main() {
    let first = LABEL;
    let middle = "runtime";
    let second = LABEL;
    return second;
}
"#;
    let program = try_build_declarations(source, |graph, _body, targets| {
        insert_constant(
            graph,
            targets,
            declaration(graph, "LABEL"),
            MirEvaluatedConstant::String("compiled".to_owned()),
        )
    })
    .expect("heap const uses");
    let (_, function) = program.functions().next().expect("main function");
    let materialized = function
        .statements()
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value)) => {
                Some((statement, value.as_str()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        materialized
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>(),
        ["compiled", "runtime", "compiled"],
        "{}",
        program.dump()
    );
    assert_eq!(source_text(source, materialized[0].0.origin), "LABEL");
    assert_eq!(source_text(source, materialized[1].0.origin), "\"runtime\"");
    assert_eq!(source_text(source, materialized[2].0.origin), "LABEL");
    for ((statement, _), expected_type) in materialized.into_iter().zip([
        MirValueType::Dynamic,
        MirValueType::Primitive(PrimitiveTag::String),
        MirValueType::Dynamic,
    ]) {
        assert_eq!(statement.effect, MirEffect::allocation());
        assert!(statement.safepoint.is_some());
        let Some(MirPlace::Temp(temp)) = statement.destination else {
            panic!("heap materialization needs a temp")
        };
        assert_eq!(
            function.temp(temp).expect("constant temp").value_type,
            expected_type
        );
    }
    assert_eq!(function.safepoints().count(), 3);
}

#[test]
fn imported_and_qualified_const_and_global_paths_share_declaration_lowering() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(101),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::config"),
        "pub const LIMIT: i64 = 7;\npub global state: i64",
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(102),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::main"),
        r#"
use game::config::LIMIT as IMPORTED_LIMIT
use game::config::state as imported_state
fn main() {
    let imported_const = IMPORTED_LIMIT;
    let qualified_const = game::config::LIMIT;
    let imported_global = imported_state;
    let qualified_global = game::config::state;
    return qualified_const;
}
"#,
    ));
    let program = try_build_declaration_graph(graph, |graph, body, targets| {
        let limit = declaration(graph, "LIMIT");
        let state = declaration(graph, "state");
        let bindings = graph.bindings_for_body(body.id).expect("main bindings");
        assert_eq!(
            body.expressions
                .values()
                .filter(|expression| {
                    bindings.resolution(expression.id)
                        == Some(&BindingResolution::Declaration(limit))
                })
                .count(),
            2
        );
        assert_eq!(
            body.expressions
                .values()
                .filter(|expression| {
                    bindings.resolution(expression.id)
                        == Some(&BindingResolution::Declaration(state))
                })
                .count(),
            2
        );
        insert_constant(
            graph,
            targets,
            limit,
            MirEvaluatedConstant::Scalar(ScalarValue::I64(7)),
        )?;
        insert_global(
            graph,
            targets,
            state,
            MirTypeContract::Primitive(PrimitiveTag::I64),
        )
    })
    .expect("imported and qualified declaration paths");
    let (_, function) = program.functions().next().expect("main function");
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::Global(MirGlobalOperation::Read { global: GLOBAL })
            ))
            .count(),
        2,
        "{}",
        program.dump()
    );
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::Assign(MirRvalue::Constant {
                    value: crate::MirImmediate::Scalar(ScalarValue::I64(7)),
                    provenance: crate::MirConstantProvenance::EvaluatedConstant,
                })
            ))
            .count(),
        2,
        "{}",
        program.dump()
    );
}

#[test]
fn global_read_operand_feeds_hostaccess_root_without_name_reresolution() {
    let source = "global state: HostRoot\nfn main() { return state.amount; }";
    let program = try_build_declarations(source, |graph, body, targets| {
        let state = declaration(graph, "state");
        let (field_expression, field) = body
            .expressions
            .values()
            .find_map(|expression| match &expression.kind {
                HirExprKind::Field(field) if field.name == "amount" => {
                    Some((expression.id, field.clone()))
                }
                _ => None,
            })
            .expect("state.amount expression");
        let access = CompileFieldAccess::new(true, true, true, true, Vec::new());
        let field_target = HostFieldTarget {
            owner: HOST_TYPE,
            semantic: HOST_FIELD_ID,
            runtime: HOST_FIELD_ID,
            access: access.clone(),
        };
        let root_origin = expression_origin(body, field.receiver);
        let field_origin = expression_origin(body, field_expression);
        targets.insert_type_descriptor(
            CompileTypeDescriptor {
                id: HOST_TYPE_ID,
                canonical_name: "declarations::HostRoot".to_owned(),
                runtime_name: "declarations::HostRoot".to_owned(),
                class: CompileTypeClass::Host {
                    runtime: HOST_RUNTIME_ID,
                },
                shape: None,
                fields: vec![HOST_FIELD_ID],
                variants: Vec::new(),
            },
            MirSourceOrigin::declaration(state, graph.declaration(state).expect("state").span),
        )?;
        targets.insert_field_descriptor(
            CompileFieldDescriptor {
                id: HOST_FIELD_ID,
                owner: HOST_TYPE_ID,
                variant: None,
                name: "amount".to_owned(),
                contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                declaration_order: 0,
                access,
                host_runtime: Some(HOST_FIELD_ID),
            },
            field_origin,
        )?;
        insert_global(graph, targets, state, MirTypeContract::Host(HOST_TYPE))?;
        targets.insert_host_path(
            ROOT_FUNCTION,
            field.receiver,
            CompileHostPathTarget {
                root: field.receiver,
                root_type: HOST_TYPE,
                segments: Vec::new(),
            },
            root_origin,
        )?;
        targets.insert_member(
            ROOT_FUNCTION,
            field_expression,
            CompileMemberTarget::HostField(field_target.clone()),
            field_origin,
        )?;
        targets.insert_host_path(
            ROOT_FUNCTION,
            field_expression,
            CompileHostPathTarget {
                root: field.receiver,
                root_type: HOST_TYPE,
                segments: vec![CompileHostPathSegment::Field(field_target)],
            },
            field_origin,
        )
    })
    .expect("global host root");
    let (_, function) = program.functions().next().expect("main function");
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let (global_index, global_temp) = statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| {
            let MirStatementKind::Global(MirGlobalOperation::Read { global: GLOBAL }) =
                statement.kind
            else {
                return None;
            };
            let Some(MirPlace::Temp(temp)) = statement.destination else {
                panic!("global read destination")
            };
            Some((index, temp))
        })
        .expect("global read");
    let (host_index, root) = statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match &statement.kind {
            MirStatementKind::Host(MirHostOperation::Read { root, .. }) => {
                Some((index, root.clone()))
            }
            _ => None,
        })
        .expect("HostAccess read");
    assert!(global_index < host_index, "{}", program.dump());
    assert_eq!(root, MirOperand::Temp(global_temp));
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(statement.kind, MirStatementKind::Global(_)))
            .count(),
        1
    );
    assert_eq!(
        source_text(source, statements[global_index].origin),
        "state"
    );
    assert_eq!(
        source_text(source, statements[host_index].origin),
        "state.amount"
    );
}

#[test]
fn declaration_path_rejects_ambiguous_or_kind_incompatible_targets_without_fallback() {
    let both = try_build_declarations(
        "const VALUE = 1\nfn main() { return VALUE; }",
        |graph, _body, targets| {
            let value = declaration(graph, "VALUE");
            insert_global(graph, targets, value, MirTypeContract::Any)?;
            insert_constant(
                graph,
                targets,
                value,
                MirEvaluatedConstant::Scalar(ScalarValue::I64(1)),
            )
        },
    )
    .expect_err("ambiguous declaration target must fail");
    assert!(
        both.to_string()
            .contains("both global and evaluated constant compile targets"),
        "{both:?}"
    );

    let wrong_kind = try_build_declarations(
        "global state: i64\nfn main() { return state; }",
        |graph, _body, targets| {
            let state = declaration(graph, "state");
            targets.insert_evaluated_constant(
                state,
                MirEvaluatedConstant::Scalar(ScalarValue::I64(1)),
                MirSourceOrigin::declaration(state, graph.declaration(state).expect("state").span),
            )
        },
    )
    .expect_err("a global cannot use const data");
    assert!(
        wrong_kind
            .to_string()
            .contains("global declaration path has an evaluated constant"),
        "{wrong_kind:?}"
    );

    let missing = try_build_declarations(
        "const VALUE = 1\nfn main() { return VALUE; }",
        |_graph, _body, _targets| Ok(()),
    )
    .expect_err("missing evaluated const must fail");
    assert!(
        missing
            .to_string()
            .contains("const declaration path has no evaluated constant"),
        "{missing:?}"
    );
}

#[test]
fn non_value_declaration_paths_do_not_gain_a_dynamic_fallback() {
    let error = try_build_declarations(
        "fn helper() {}\nfn main() { return helper; }",
        |_graph, _body, _targets| Ok(()),
    )
    .expect_err("function declaration value path is unsupported");
    assert!(
        error
            .to_string()
            .contains("declaration value path does not name a const or global"),
        "{error:?}"
    );
}

fn source_text(source: &str, origin: MirSourceOrigin) -> &str {
    &source[origin.span.start as usize..origin.span.end as usize]
}

fn expression_origin(body: &HirBody, expression: vela_hir::ids::HirExprId) -> MirSourceOrigin {
    MirSourceOrigin::expression(
        body.id,
        expression,
        body.expression(expression).expect("expression").origin.span,
    )
}

#[test]
fn fixture_declaration_kinds_are_the_expected_semantic_inputs() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(99),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("declaration_kinds"),
        "const VALUE = 1\nglobal state: i64\nfn main() {}",
    ));
    graph.resolve_imports();
    assert_eq!(
        graph
            .declarations()
            .map(|declaration| (declaration.name.as_str(), declaration.kind))
            .collect::<Vec<_>>(),
        [
            ("VALUE", DeclarationKind::Const),
            ("state", DeclarationKind::Global),
            ("main", DeclarationKind::Function),
        ]
    );
}
