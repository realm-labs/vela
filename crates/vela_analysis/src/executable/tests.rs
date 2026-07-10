use vela_common::{PrimitiveTag, SourceId, Span};
use vela_def::FunctionId;
use vela_hir::body::{HirBinaryOp, HirBodyOwner, HirExprKind, HirLiteral, HirPatternKind};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, ModulePath, ModuleSource};

use super::{ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableReceiverInput};
use crate::facts::AnalysisFacts;
use crate::literals::{LiteralPrimitiveContext, ResolvedLiteralFact};
use crate::semantic_facts::{
    CallTargetFact, MemberTargetFact, OperatorTargetFact, ScriptTypeTargetFact,
};
use crate::type_fact::TypeFact;

#[test]
fn shared_trait_default_facts_are_qualified_by_concrete_executable() {
    let source = SourceId::new(71);
    let text = r#"
trait Probe {
    fn target(self);
    fn inspect(self, fallback = self.value) {
        let current = self;
        let nested = |ignored| self.value;
        current.value;
        self.target();
        nested(fallback);
    }
}

struct Player { value: i64 }
struct Monster { value: String }

impl Probe for Player { fn target(self) {} }
impl Probe for Monster { fn target(self) {} }

fn unrelated() { return 99; }
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let probe = declaration_named(&graph, "Probe");
    let player = declaration_named(&graph, "Player");
    let monster = declaration_named(&graph, "Monster");
    let inspect = graph
        .trait_shape(probe)
        .expect("Probe trait shape")
        .methods
        .iter()
        .find(|method| method.name == "inspect")
        .expect("inspect method");
    let node = inspect
        .default_body_node
        .expect("inspect default body node");
    let body = graph
        .trait_default_method_body(node)
        .expect("inspect default body");
    let bindings = graph
        .trait_default_method_bindings(node)
        .expect("inspect default bindings");
    let [current] = bindings.locals_named("current") else {
        panic!("current local")
    };
    let lambda = graph
        .bodies()
        .find(|candidate| {
            matches!(candidate.owner, HirBodyOwner::Lambda { parent, .. } if parent == body.id)
        })
        .expect("nested lambda body");
    let default = body.params[1].default_body.expect("fallback default body");

    let direct_self = expression_inside(&graph, source, text, "let current = self", "self");
    let current_value = expression_exact(&graph, source, text, "current.value");
    let nested_value = expression_inside(
        &graph,
        source,
        text,
        "let nested = |ignored| self.value",
        "self.value",
    );
    let default_value =
        expression_inside(&graph, source, text, "fallback = self.value", "self.value");
    let target_call = expression_exact(&graph, source, text, "self.target()");
    let unrelated_literal = expression_exact(&graph, source, text, "99");

    let player_function = FunctionId::new(701);
    let monster_function = FunctionId::new(702);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(player_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Player"))
                    .with_script_type(ScriptTypeTargetFact::declaration(player)),
            ),
            ExecutableAnalysisInput::new(monster_function, body.id).with_receiver(
                ExecutableReceiverInput::new(TypeFact::record("game::Monster"))
                    .with_script_type(ScriptTypeTargetFact::declaration(monster)),
            ),
        ],
    )
    .expect("qualified executable analysis");
    let player_view = generation.view(player_function).expect("Player view");
    let monster_view = generation.view(monster_function).expect("Monster view");

    assert_eq!(player_view.root_body(), monster_view.root_body());
    assert!(player_view.contains_body(lambda.id));
    assert!(player_view.contains_body(default));
    assert!(monster_view.contains_body(lambda.id));
    assert!(monster_view.contains_body(default));

    assert_eq!(
        player_view.expression(direct_self),
        Some(&TypeFact::record("game::Player"))
    );
    assert_eq!(
        monster_view.expression(direct_self),
        Some(&TypeFact::record("game::Monster"))
    );
    assert_eq!(
        player_view.local(*current),
        Some(&TypeFact::record("game::Player"))
    );
    assert_eq!(
        monster_view.local(*current),
        Some(&TypeFact::record("game::Monster"))
    );

    assert_script_field(&player_view, current_value, player, TypeFact::I64);
    assert_script_field(&monster_view, current_value, monster, TypeFact::STRING);
    assert_script_field(&player_view, nested_value, player, TypeFact::I64);
    assert_script_field(&monster_view, nested_value, monster, TypeFact::STRING);
    assert_script_field(&player_view, default_value, player, TypeFact::I64);
    assert_script_field(&monster_view, default_value, monster, TypeFact::STRING);

    let player_target = impl_method_node(&graph, "Player", "target");
    let monster_target = impl_method_node(&graph, "Monster", "target");
    assert_ne!(player_target, monster_target);
    assert_eq!(
        player_view.call_target(target_call),
        Some(&CallTargetFact::ScriptMethod {
            method: player_target
        })
    );
    assert_eq!(
        monster_view.call_target(target_call),
        Some(&CallTargetFact::ScriptMethod {
            method: monster_target
        })
    );

    assert!(player_view.expression(unrelated_literal).is_none());
    assert!(monster_view.expression(unrelated_literal).is_none());
    let editor = AnalysisFacts::from_module_graph(&graph);
    assert_ne!(
        editor.expression(direct_self),
        Some(&TypeFact::record("game::Player"))
    );

    let root_block = match body.root {
        vela_hir::body::HirBodyRoot::Block(block) => block,
        _ => panic!("inspect block body"),
    };
    assert!(player_view.block_control_flow(root_block).is_some());
    let statement = *body.statements.keys().next().expect("inspect statement");
    assert!(player_view.statement_control_flow(statement).is_some());
}

#[test]
fn contextual_literals_rebuild_scoped_semantic_operator_facts() {
    let source = SourceId::new(72);
    let text = "fn add(value: i32) -> i32 { return value + 300; }";
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let add = declaration_named(&graph, "add");
    let body = graph.function_body(add).expect("add body");
    let literal = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Literal(_)))
        .expect("numeric literal")
        .id;
    let binary = body
        .expressions
        .values()
        .find(|expression| matches!(expression.kind, HirExprKind::Binary { .. }))
        .expect("binary expression")
        .id;
    let typed = FunctionId::new(711);
    let dynamic = FunctionId::new(712);
    let out_of_range = FunctionId::new(713);
    let mut generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [
            ExecutableAnalysisInput::new(typed, body.id).with_literal_context(
                literal,
                LiteralPrimitiveContext::Expected(PrimitiveTag::I32),
            ),
            ExecutableAnalysisInput::new(dynamic, body.id)
                .with_literal_context(literal, LiteralPrimitiveContext::DeferredDynamic),
            ExecutableAnalysisInput::new(out_of_range, body.id)
                .with_literal_context(literal, LiteralPrimitiveContext::Expected(PrimitiveTag::U8)),
        ],
    )
    .expect("literal-qualified analysis");

    let typed_view = generation.view(typed).expect("typed view");
    assert_eq!(typed_view.expression(literal), Some(&TypeFact::I32));
    assert!(matches!(
        typed_view.literal(literal),
        Some(Ok(ResolvedLiteralFact::Scalar(value))) if value.primitive() == PrimitiveTag::I32
    ));
    assert_eq!(
        typed_view.operator_target(binary),
        Some(OperatorTargetFact::Binary(HirBinaryOp::Add))
    );

    let dynamic_view = generation.view(dynamic).expect("dynamic view");
    assert_eq!(dynamic_view.expression(literal), Some(&TypeFact::Any));
    assert!(matches!(
        dynamic_view.literal(literal),
        Some(Ok(ResolvedLiteralFact::Deferred(_)))
    ));
    assert_eq!(
        dynamic_view.operator_target(binary),
        Some(OperatorTargetFact::Dynamic)
    );
    assert!(typed_view.literal_diagnostics(&graph).is_empty());
    assert_eq!(
        generation
            .view(out_of_range)
            .expect("out-of-range view")
            .literal_diagnostics(&graph)
            .len(),
        1
    );

    generation
        .rebuild_literal_contexts(
            &graph,
            typed,
            [(literal, LiteralPrimitiveContext::DeferredDynamic)],
        )
        .expect("second-pass literal rebuild");
    let rebuilt = generation.view(typed).expect("rebuilt typed view");
    assert_eq!(rebuilt.expression(literal), Some(&TypeFact::Any));
    assert_eq!(
        rebuilt.operator_target(binary),
        Some(OperatorTargetFact::Dynamic)
    );
}

#[test]
fn executable_views_expose_validated_pattern_literal_facts() {
    let source = SourceId::new(73);
    let text = r#"
fn main(value) {
    return match value {
        128i8 => 1,
        _ => 0,
    };
}
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = declaration_named(&graph, "main");
    let body = graph.function_body(main).expect("main body");
    let pattern = body
        .patterns
        .values()
        .find(|pattern| {
            matches!(
                &pattern.kind,
                HirPatternKind::Literal(Some(HirLiteral::Integer(literal)))
                    if literal.text == "128"
            )
        })
        .expect("invalid numeric pattern")
        .id;
    let function = FunctionId::new(714);
    let generation = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("executable pattern analysis");
    let view = generation.view(function).expect("main view");

    assert!(matches!(view.pattern_literal(pattern), Some(Err(_))));
    assert_eq!(view.pattern(pattern), Some(&TypeFact::Unknown));
    let diagnostics = view.literal_diagnostics(&graph);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one pattern literal diagnostic");
    };
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::invalid_int_literal")
    );
    let span = diagnostic.span.expect("pattern diagnostic span");
    assert_eq!(&text[span.start as usize..span.end as usize], "128i8");
}

fn assert_script_field(
    view: &super::ExecutableAnalysisView<'_>,
    expression: HirExprId,
    owner: vela_hir::ids::HirDeclId,
    fact: TypeFact,
) {
    assert_eq!(view.expression(expression), Some(&fact));
    assert_eq!(
        view.member_target(expression),
        Some(&MemberTargetFact::ScriptField {
            owner,
            variant: None,
            name: "value".to_owned(),
        })
    );
}

fn declaration_named(graph: &ModuleGraph, name: &str) -> vela_hir::ids::HirDeclId {
    graph
        .declarations()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.id)
        .unwrap_or_else(|| panic!("{name} declaration"))
}

fn impl_method_node(graph: &ModuleGraph, owner: &str, method: &str) -> vela_hir::ids::HirNodeId {
    graph
        .declarations_by_kind(DeclarationKind::Impl)
        .into_iter()
        .filter_map(|declaration| graph.impl_metadata(declaration.id))
        .find(|metadata| {
            metadata
                .target_path
                .last()
                .is_some_and(|name| name == owner)
        })
        .and_then(|metadata| {
            metadata
                .methods
                .iter()
                .find(|candidate| candidate.name == method)
        })
        .map(|method| method.node)
        .unwrap_or_else(|| panic!("{owner}::{method} node"))
}

fn expression_exact(
    graph: &ModuleGraph,
    source: SourceId,
    text: &str,
    expression: &str,
) -> HirExprId {
    let start = text.find(expression).expect("expression source offset");
    expression_at(graph, source, start, expression.len())
}

fn expression_inside(
    graph: &ModuleGraph,
    source: SourceId,
    text: &str,
    enclosing: &str,
    expression: &str,
) -> HirExprId {
    let enclosing_start = text.find(enclosing).expect("enclosing source offset");
    let relative = enclosing.find(expression).expect("inner source offset");
    expression_at(graph, source, enclosing_start + relative, expression.len())
}

fn expression_at(graph: &ModuleGraph, source: SourceId, start: usize, length: usize) -> HirExprId {
    graph
        .expression_at_span(Span::new(
            source,
            u32::try_from(start).expect("test source offset"),
            u32::try_from(start + length).expect("test source end"),
        ))
        .expect("HIR expression at source span")
}
