use std::collections::{BTreeMap, BTreeSet};
use vela_package::ModulePath;

use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::SourceId;
use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::body::{HirBody, HirBodyOwner, HirExprKind};
use vela_hir::ids::{HirBodyId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileFunctionIdentity, CompileLambdaParameterTarget,
    CompileLambdaTarget, CompileParameter, CompileParameterDefault, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, CompileTargetSnapshotBuilder, CompileTryFamily,
    CompileTryLayoutTarget, CompileTryTarget, CompileTypeClass, CompileTypeDescriptor,
    CompileVariantDescriptor, MirBuildError, MirEffect, MirFieldTarget, MirFunction,
    MirLoweringConfig, MirLoweringInput, MirOperand, MirSourceOrigin, MirStatementKind,
    MirTerminatorKind, MirTryContinue,
};

const SOURCE: SourceId = SourceId::new(92);
const FUNCTION: FunctionId = FunctionId::new(9_200);

const OPTION_TYPE: TypeId = TypeId::new(9_210);
const OPTION_SOME: VariantId = VariantId::new(9_211);
const OPTION_NONE: VariantId = VariantId::new(9_212);
const OPTION_VALUE: FieldId = FieldId::new(9_213);
const OPTION_LAYOUT: CompileTryLayoutTarget = CompileTryLayoutTarget {
    family: CompileTryFamily::Option,
    type_id: OPTION_TYPE,
    continue_variant: OPTION_SOME,
    break_variant: OPTION_NONE,
    continue_payload: OPTION_VALUE,
};

const RESULT_TYPE: TypeId = TypeId::new(9_220);
const RESULT_OK: VariantId = VariantId::new(9_221);
const RESULT_ERR: VariantId = VariantId::new(9_222);
const RESULT_VALUE: FieldId = FieldId::new(9_223);
const RESULT_ERROR: FieldId = FieldId::new(9_224);
const RESULT_LAYOUT: CompileTryLayoutTarget = CompileTryLayoutTarget {
    family: CompileTryFamily::Result,
    type_id: RESULT_TYPE,
    continue_variant: RESULT_OK,
    break_variant: RESULT_ERR,
    continue_payload: RESULT_VALUE,
};

#[derive(Clone, Copy, Debug)]
struct TryExpression {
    body: HirBodyId,
    expression: HirExprId,
    origin: MirSourceOrigin,
}

struct BuiltFixture {
    program: crate::MirProgram,
    root: HirBodyId,
    defaults: Vec<HirBodyId>,
    lambdas: Vec<HirBodyId>,
    tries: Vec<TryExpression>,
}

fn build(
    source: &str,
    target_for: impl Fn(&HirBody, HirExprId) -> Option<CompileTryTarget>,
) -> Result<BuiltFixture, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SOURCE,
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("try_builder"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let root = graph.function_body(declaration.id).expect("main body");
    let bindings = graph.bindings_for_body(root.id).expect("main bindings");
    let parameters = root
        .params
        .iter()
        .map(|parameter| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone(),
            contract: None,
            default: parameter.default_body.map_or(
                CompileParameterDefault::Required,
                CompileParameterDefault::HirBody,
            ),
            origin: Some(MirSourceOrigin::body(root.id, parameter.origin.span)),
        })
        .collect::<Vec<_>>();
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(FUNCTION, root.id)],
    )
    .expect("try analysis generation");
    let root_origin = MirSourceOrigin::body(root.id, root.origin.span);
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        root.id,
        CompileFunctionDescriptor {
            id: FUNCTION,
            class: CompileFunctionClass::Script,
            canonical_symbol: "try_builder::main".to_owned(),
            debug_name: "main".to_owned(),
            signature: CompileSignature {
                asyncness: vela_common::CallableAsyncness::Sync,
                parameters,
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            },
            access: CompileFunctionAccess::script(false),
        },
        root_origin,
    )?;

    let lambdas = insert_lambda_targets(&graph, root.id, &mut targets)?;
    let defaults = root
        .params
        .iter()
        .filter_map(|parameter| parameter.default_body)
        .collect::<Vec<_>>();
    let mut tries = graph
        .bodies()
        .filter(|body| {
            graph
                .body_and_ancestors(body.id)
                .any(|ancestor| ancestor.id == root.id)
        })
        .flat_map(|body| {
            body.expressions
                .iter()
                .filter_map(move |(expression, record)| {
                    matches!(record.kind, HirExprKind::Try { .. }).then_some((
                        body,
                        *expression,
                        MirSourceOrigin::expression(body.id, *expression, record.origin.span),
                    ))
                })
        })
        .collect::<Vec<_>>();
    tries.sort_unstable_by_key(|(body, expression, origin)| {
        (
            origin.span.source,
            origin.span.start,
            origin.span.end,
            body.id,
            *expression,
        )
    });
    let selected = tries
        .iter()
        .filter_map(|(body, expression, origin)| {
            target_for(body, *expression).map(|target| (*expression, *origin, target))
        })
        .collect::<Vec<_>>();
    let families = selected
        .iter()
        .flat_map(|(_, _, target)| match target {
            CompileTryTarget::Expected(layout) => vec![layout.family],
            CompileTryTarget::Dynamic { option, result } => {
                vec![option.family, result.family]
            }
        })
        .collect::<BTreeSet<_>>();
    for family in families {
        insert_try_layout(&mut targets, family, root_origin)?;
    }
    for (expression, origin, target) in selected {
        targets.insert_try_target(FUNCTION, expression, target, origin)?;
    }

    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(FUNCTION),
        root.id,
        analysis.view(FUNCTION).expect("try analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;
    let program = crate::build_mir(input)?;
    Ok(BuiltFixture {
        program,
        root: root.id,
        defaults,
        lambdas,
        tries: tries
            .into_iter()
            .map(|(body, expression, origin)| TryExpression {
                body: body.id,
                expression,
                origin,
            })
            .collect(),
    })
}

fn insert_lambda_targets(
    graph: &ModuleGraph,
    root: HirBodyId,
    targets: &mut CompileTargetSnapshotBuilder,
) -> Result<Vec<HirBodyId>, MirBuildError> {
    let mut bodies = graph
        .bodies()
        .filter(|body| matches!(body.owner, HirBodyOwner::Lambda { .. }))
        .filter(|body| {
            graph
                .body_and_ancestors(body.id)
                .any(|ancestor| ancestor.id == root)
        })
        .collect::<Vec<_>>();
    bodies.sort_unstable_by_key(|body| {
        let depth = graph
            .body_and_ancestors(body.id)
            .filter(|ancestor| matches!(ancestor.owner, HirBodyOwner::Lambda { .. }))
            .count();
        (depth, body.origin.span.start, body.id)
    });
    let mut symbols = BTreeMap::from([(root, "try_builder::main".to_owned())]);
    for body in &bodies {
        let HirBodyOwner::Lambda { parent, expression } = body.owner else {
            unreachable!("lambda list was filtered")
        };
        let parent = graph
            .body_and_ancestors(parent)
            .find(|candidate| {
                candidate.id == root || matches!(candidate.owner, HirBodyOwner::Lambda { .. })
            })
            .expect("lambda executable parent")
            .id;
        let symbol = format!(
            "{}::<lambda@{}>",
            symbols.get(&parent).expect("parent lambda symbol"),
            body.origin.span.start
        );
        let bindings = graph.bindings_for_body(body.id).expect("lambda bindings");
        let parameters = body
            .params
            .iter()
            .map(|parameter| CompileLambdaParameterTarget {
                parameter: parameter.id,
                local: parameter.local,
                name: bindings
                    .local(parameter.local)
                    .expect("lambda parameter binding")
                    .name
                    .clone(),
                contract: None,
                origin: MirSourceOrigin::body(body.id, parameter.origin.span),
            })
            .collect();
        targets.insert_lambda(
            FUNCTION,
            CompileLambdaTarget {
                body: body.id,
                parent,
                expression,
                code_symbol: symbol.clone(),
                parameters,
                origin: MirSourceOrigin::body(body.id, body.origin.span),
            },
        )?;
        symbols.insert(body.id, symbol);
    }
    Ok(bodies.into_iter().map(|body| body.id).collect())
}

fn insert_try_layout(
    targets: &mut CompileTargetSnapshotBuilder,
    family: CompileTryFamily,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    let (layout, type_name, continue_name, break_name, break_fields) = match family {
        CompileTryFamily::Option => (OPTION_LAYOUT, "std::Option", "Some", "None", Vec::new()),
        CompileTryFamily::Result => (
            RESULT_LAYOUT,
            "std::Result",
            "Ok",
            "Err",
            vec![RESULT_ERROR],
        ),
    };
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: layout.type_id,
            canonical_name: type_name.to_owned(),
            runtime_name: type_name.to_owned(),
            class: CompileTypeClass::Standard,
            shape: None,
            fields: Vec::new(),
            variants: vec![layout.continue_variant, layout.break_variant],
        },
        origin,
    )?;
    targets.insert_variant_descriptor(
        CompileVariantDescriptor {
            id: layout.continue_variant,
            owner: layout.type_id,
            name: continue_name.to_owned(),
            fields: vec![layout.continue_payload],
            declaration_order: 0,
        },
        origin,
    )?;
    targets.insert_variant_descriptor(
        CompileVariantDescriptor {
            id: layout.break_variant,
            owner: layout.type_id,
            name: break_name.to_owned(),
            fields: break_fields.clone(),
            declaration_order: 1,
        },
        origin,
    )?;
    insert_field(
        targets,
        layout.continue_payload,
        layout.type_id,
        layout.continue_variant,
        origin,
    )?;
    if let Some(field) = break_fields.first() {
        insert_field(
            targets,
            *field,
            layout.type_id,
            layout.break_variant,
            origin,
        )?;
    }
    Ok(())
}

fn insert_field(
    targets: &mut CompileTargetSnapshotBuilder,
    field: FieldId,
    owner: TypeId,
    variant: VariantId,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: field,
            owner,
            variant: Some(variant),
            name: "0".to_owned(),
            contract: None,
            declaration_order: 0,
            access: CompileFieldAccess::script(),
            host_runtime: None,
        },
        origin,
    )
}

fn only_function(fixture: &BuiltFixture) -> &MirFunction {
    let [function] = fixture.program.functions_for_body(fixture.root) else {
        panic!("root body must have exactly one MIR function")
    };
    fixture
        .program
        .function(*function)
        .expect("defined root function")
}

#[derive(Clone, Debug)]
struct TryRegion {
    value: MirOperand,
    target: CompileTryTarget,
    result: crate::MirLocalId,
    continuations: Vec<MirTryContinue>,
    propagate: crate::MirBlockId,
    invalid: crate::MirBlockId,
    join: crate::MirBlockId,
}

fn try_regions(function: &MirFunction) -> Vec<TryRegion> {
    function
        .blocks()
        .filter_map(|(_, block)| {
            let terminator = block.terminator()?;
            let MirTerminatorKind::TrySwitch {
                value,
                target,
                result,
                continuations,
                propagate,
                invalid,
                join,
            } = &terminator.kind
            else {
                return None;
            };
            Some(TryRegion {
                value: value.clone(),
                target: *target,
                result: *result,
                continuations: continuations.clone(),
                propagate: *propagate,
                invalid: *invalid,
                join: *join,
            })
        })
        .collect()
}

#[test]
fn mir_builder_expected_option_try_is_explicit_cfg() {
    let fixture = build(
        "fn main(value) { let inner = value?; return inner; }",
        |_, _| Some(CompileTryTarget::Expected(OPTION_LAYOUT)),
    )
    .expect("expected Option try MIR");
    assert_eq!(
        fixture.program.dump(),
        r#"mir {
  target function#9200 CompileFunctionDescriptor { id: FunctionId(9200), class: Script, canonical_symbol: "try_builder::main", debug_name: "main", signature: CompileSignature { asyncness: Sync, parameters: [CompileParameter { name: "value", contract: None, default: Required, origin: Some(MirSourceOrigin { body: Some(HirBodyId(0)), node: Body(HirBodyId(0)), span: Span { source: SourceId(92), start: 8, end: 13 } }) }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  target type#9210 CompileTypeDescriptor { id: TypeId(9210), canonical_name: "std::Option", runtime_name: "std::Option", class: Standard, shape: None, fields: [], variants: [VariantId(9211), VariantId(9212)] }
  target variant#9211 CompileVariantDescriptor { id: VariantId(9211), owner: TypeId(9210), name: "Some", fields: [FieldId(9213)], declaration_order: 0 }
  target variant#9212 CompileVariantDescriptor { id: VariantId(9212), owner: TypeId(9210), name: "None", fields: [], declaration_order: 1 }
  target field#9213 CompileFieldDescriptor { id: FieldId(9213), owner: TypeId(9210), variant: Some(VariantId(9211)), name: "0", contract: None, declaration_order: 0, access: CompileFieldAccess { readable: true, writable: true, reflect_readable: true, reflect_writable: true, required_permissions: [] }, host_runtime: None }
  fn f0 body h0 owner function#9200 symbol="try_builder::main" @92:15..52/h0 {
    param p0: value -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @92:8..13/h0
    local l0: Script(HirLocalId(0)) Dynamic @92:8..13/h0
    local l1: Script(HirLocalId(1)) Dynamic @92:21..26/h0
    local l2: Synthetic Dynamic @92:29..35/e0
    debug dl0: value -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @92:8..13/h0
    debug dl1: inner -> l1 kind=Local hir=Some(1) scope=h0 live=[] @92:21..26/h0
    bb0:
      -> try.switch l0 target=Expected(CompileTryLayoutTarget { family: Option, type_id: TypeId(9210), continue_variant: VariantId(9211), break_variant: VariantId(9212), continue_payload: FieldId(9213) }) result=l2 continuations=[MirTryContinue { layout: CompileTryLayoutTarget { family: Option, type_id: TypeId(9210), continue_variant: VariantId(9211), break_variant: VariantId(9212), continue_payload: FieldId(9213) }, block: MirBlockId(4) }] propagate=bb2 invalid=bb3 join=bb1 [pure] @92:29..35/e0
    bb1:
      s1: l1 = l2 [pure] @92:17..36/s0
      -> return l1 [pure] @92:37..50/s1
    bb2:
      -> return l0 [pure] @92:29..35/e0
    bb3:
      -> try.type-mismatch target=Expected(CompileTryLayoutTarget { family: Option, type_id: TypeId(9210), continue_variant: VariantId(9211), break_variant: VariantId(9212), continue_payload: FieldId(9213) }) [trap] @92:29..35/e0
    bb4:
      s0: l2 = field.read l0 VariantSlot { type_id: TypeId(9210), variant: VariantId(9211), field: FieldId(9213) } [trap] @92:29..35/e0
      -> jump bb1 [pure] @92:29..35/e0
  }
}
"#
    );
    let function = only_function(&fixture);
    let regions = try_regions(function);
    let [region] = regions.as_slice() else {
        panic!("expected one canonical try region")
    };
    assert_eq!(region.target, CompileTryTarget::Expected(OPTION_LAYOUT));
    assert_eq!(region.continuations.len(), 1);
    assert_eq!(region.continuations[0].layout, OPTION_LAYOUT);
    assert!(matches!(region.value, MirOperand::Local(_)));
    assert!(function.local(region.result).is_some());
    assert!(function.block(region.join).is_some());
    assert_ne!(region.join, region.propagate);
    assert_ne!(region.join, region.invalid);
    let invalid = function
        .block(region.invalid)
        .and_then(|block| block.terminator())
        .expect("invalid try block");
    assert!(matches!(
        &invalid.kind,
        MirTerminatorKind::TryTypeMismatch { target }
            if *target == CompileTryTarget::Expected(OPTION_LAYOUT)
    ));
    assert_eq!(invalid.origin, fixture.tries[0].origin);
    assert!(invalid.effect.may_trap);
    assert!(invalid.safepoint.is_none());

    let reads = function
        .statements()
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::ReadField { target, .. } => Some((
                target.clone(),
                statement.origin,
                statement.effect,
                statement.safepoint,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 1);
    assert_eq!(
        reads[0].0,
        MirFieldTarget::VariantSlot {
            type_id: OPTION_TYPE,
            variant: OPTION_SOME,
            field: OPTION_VALUE,
        }
    );
    assert_eq!(reads[0].1, fixture.tries[0].origin);
    assert!(reads[0].2.may_trap);
    assert!(reads[0].3.is_none());
    assert_eq!(
        function
            .blocks()
            .filter_map(|(_, block)| block.terminator())
            .filter(|terminator| matches!(terminator.kind, MirTerminatorKind::Return(_)))
            .count(),
        2,
        "the source return and propagation return must remain distinct"
    );
}

#[test]
fn mir_builder_expected_result_uses_its_authoritative_layout() {
    let fixture = build(
        "fn main(value) { let inner = value?; return inner; }",
        |_, _| Some(CompileTryTarget::Expected(RESULT_LAYOUT)),
    )
    .expect("expected Result try MIR");
    let function = only_function(&fixture);
    let regions = try_regions(function);
    let [region] = regions.as_slice() else {
        panic!("expected one try switch")
    };
    assert_eq!(region.target, CompileTryTarget::Expected(RESULT_LAYOUT));
    assert_eq!(region.continuations[0].layout, RESULT_LAYOUT);
    assert!(function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::ReadField {
            target: MirFieldTarget::VariantSlot {
                type_id: RESULT_TYPE,
                variant: RESULT_OK,
                field: RESULT_VALUE,
            },
            ..
        }
    )));
}

#[test]
fn mir_builder_dynamic_try_discriminates_both_families_explicitly() {
    let fixture = build(
        "fn main(value) { let inner = value?; return inner; }",
        |_, _| {
            Some(CompileTryTarget::Dynamic {
                option: OPTION_LAYOUT,
                result: RESULT_LAYOUT,
            })
        },
    )
    .expect("dynamic try MIR");
    let function = only_function(&fixture);
    let dynamic_regions = try_regions(function);
    let [region] = dynamic_regions.as_slice() else {
        panic!("expected one dynamic try switch")
    };
    assert_eq!(
        region.target,
        CompileTryTarget::Dynamic {
            option: OPTION_LAYOUT,
            result: RESULT_LAYOUT,
        }
    );
    assert_eq!(
        region
            .continuations
            .iter()
            .map(|continuation| continuation.layout)
            .collect::<Vec<_>>(),
        [OPTION_LAYOUT, RESULT_LAYOUT]
    );
    let propagation = function
        .block(region.propagate)
        .and_then(|block| block.terminator())
        .expect("shared propagation return");
    assert!(matches!(
        &propagation.kind,
        MirTerminatorKind::Return(Some(value)) if value == &region.value
    ));
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::ReadField { .. }))
            .count(),
        2
    );
}

#[test]
fn mir_builder_nested_try_and_effectful_operand_are_evaluated_once() {
    let nested = build(
        "fn main(value) { let inner = value??; return inner; }",
        |_, _| Some(CompileTryTarget::Expected(OPTION_LAYOUT)),
    )
    .expect("nested try MIR");
    assert_eq!(try_regions(only_function(&nested)).len(), 2);
    assert_eq!(nested.tries.len(), 2);
    assert_ne!(nested.tries[0].expression, nested.tries[1].expression);
    assert_ne!(nested.tries[0].origin.span, nested.tries[1].origin.span);

    let assigned = build(
        "fn main(value, replacement) { let inner = (value = replacement)?; return inner; }",
        |_, _| {
            Some(CompileTryTarget::Dynamic {
                option: OPTION_LAYOUT,
                result: RESULT_LAYOUT,
            })
        },
    )
    .expect("effectful try operand MIR");
    let function = only_function(&assigned);
    let value = function.parameters()[0].storage;
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| statement.destination == Some(crate::MirPlace::local(value)))
            .count(),
        1,
        "the assignment operand must execute exactly once before the switch"
    );
}

#[test]
fn mir_builder_try_targets_survive_defaults_root_and_lambda_bodies() {
    let fixture = build(
        r#"
fn main(fallback, value = fallback?) {
    let unwrap = || value?;
    let inner = fallback?;
    return unwrap;
}
"#,
        |body, _| match body.owner {
            HirBodyOwner::ParameterDefault { .. } => {
                Some(CompileTryTarget::Expected(OPTION_LAYOUT))
            }
            HirBodyOwner::Lambda { .. } => Some(CompileTryTarget::Dynamic {
                option: OPTION_LAYOUT,
                result: RESULT_LAYOUT,
            }),
            HirBodyOwner::Declaration(_) => Some(CompileTryTarget::Expected(RESULT_LAYOUT)),
            HirBodyOwner::ConstInitializer(_)
            | HirBodyOwner::SchemaFieldDefault(_)
            | HirBodyOwner::TraitDefaultMethod(_)
            | HirBodyOwner::ImplMethod(_) => None,
        },
    )
    .expect("default/root/lambda try MIR");
    assert_eq!(fixture.defaults.len(), 1);
    assert_eq!(fixture.lambdas.len(), 1);
    assert!(
        fixture
            .program
            .functions_for_body(fixture.defaults[0])
            .is_empty(),
        "a parameter default remains prologue CFG, not a MIR function"
    );
    let root = only_function(&fixture);
    let root_switches = root
        .blocks()
        .filter_map(|(_, block)| block.terminator())
        .filter_map(|terminator| match &terminator.kind {
            MirTerminatorKind::TrySwitch { continuations, .. } => {
                Some((terminator.origin, continuations.len()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(root_switches.len(), 2);
    assert!(
        root_switches.contains(&(
            fixture
                .tries
                .iter()
                .find(|try_expression| try_expression.body == fixture.defaults[0])
                .expect("default try")
                .origin,
            1,
        ))
    );
    assert!(
        root_switches.contains(&(
            fixture
                .tries
                .iter()
                .find(|try_expression| try_expression.body == fixture.root)
                .expect("root try")
                .origin,
            1,
        ))
    );

    let [lambda] = fixture.program.functions_for_body(fixture.lambdas[0]) else {
        panic!("lambda body must own one MIR function")
    };
    let lambda = fixture
        .program
        .function(*lambda)
        .expect("lambda MIR function");
    let lambda_switches = try_regions(lambda);
    let [dynamic] = lambda_switches.as_slice() else {
        panic!("lambda should contain one dynamic try switch")
    };
    assert_eq!(dynamic.continuations.len(), 2);
}

#[test]
fn mir_builder_rejects_try_without_an_authoritative_target() {
    let error = match build(
        "fn main(value) { let inner = value?; return inner; }",
        |_, _| None,
    ) {
        Ok(_) => panic!("targetless try must fail MIR construction"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("try expression has no compile target"),
        "{error:?}"
    );
    assert!(error.origin().is_some());
}
