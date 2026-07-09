use super::*;
use crate::body::{HirBodyOwner, HirBodyRoot, HirScopeKind, HirStmtKind};

fn hir_resolution_for_span<'a>(
    graph: &ModuleGraph,
    bindings: &'a BindingMap,
    span: Span,
) -> Option<&'a BindingResolution> {
    let expression = graph.expression_at_span(span)?;
    bindings.resolution(expression)
}

#[test]
fn function_bindings_resolve_params_and_locals_with_expression_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::player",
        r#"
fn main(player) {
    let next = player.level;
    return next;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [player] = bindings.locals_named("player") else {
        panic!("expected one player binding");
    };
    let [next] = bindings.locals_named("next") else {
        panic!("expected one next binding");
    };
    assert_eq!(
        bindings.local(*player).map(|local| local.kind),
        Some(LocalBindingKind::Parameter)
    );
    assert_eq!(
        bindings.local(*next).map(|local| local.kind),
        Some(LocalBindingKind::Let)
    );
    assert!(
        graph
            .function_body(main)
            .is_some_and(|body| body.expressions.len() >= 2)
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(*player))
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(*next))
    );
}
#[test]
fn binding_unresolved_names_report_candidate_hints() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::player",
        r#"
fn main(player) {
    return plaeyr;
}
"#,
    ));
    let unresolved = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_name"))
        .expect("unresolved name diagnostic");
    assert_eq!(unresolved.labels.len(), 2);
    assert_eq!(unresolved.labels[0].message, "did you mean `player`?");
    assert_eq!(
        unresolved.labels[1].message,
        "candidate `player` is declared here"
    );
    assert_ne!(unresolved.labels[0].span, unresolved.labels[1].span);
}
#[test]
fn binding_tracks_nested_for_and_lambda_scopes() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(rewards) {
    for reward in rewards {
        let mapper = |reward| reward.count;
    }
    return rewards;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let reward_bindings = bindings.locals_named("reward");
    assert_eq!(reward_bindings.len(), 2);
    assert_eq!(
        bindings.local(reward_bindings[0]).map(|local| local.kind),
        Some(LocalBindingKind::For)
    );
    assert_eq!(
        bindings.local(reward_bindings[1]).map(|local| local.kind),
        Some(LocalBindingKind::LambdaParameter)
    );
}

#[test]
fn binding_resolves_lambda_capture_paths_at_source_spans() {
    let mut graph = ModuleGraph::new();
    let text = r#"
fn main(player) {
    let even = 0;
    let mapper = |value| if value > 0 { even } else { player.level };
    return mapper;
}
"#;
    let module = graph.add_source(source(1, "game::reward", text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [even] = bindings.locals_named("even") else {
        panic!("expected one even binding");
    };
    let [player] = bindings.locals_named("player") else {
        panic!("expected one player binding");
    };

    let let_start = text.find("let even").expect("even let");
    let let_end = let_start + "let even = 0;".len();
    assert_eq!(
        bindings.local_named_at(
            "even",
            LocalBindingKind::Let,
            Span::new(SourceId::new(1), let_start as u32, let_end as u32)
        ),
        Some(*even)
    );
    let even_start = text.find("{ even }").expect("even capture") + "{ ".len();
    let player_start = text.find("{ player.level }").expect("player capture") + "{ ".len();
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                even_start as u32,
                (even_start + "even".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*even))
    );
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                player_start as u32,
                (player_start + "player".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*player))
    );
}

#[test]
fn binding_resolves_if_expression_lambda_captures_at_vm_spans() {
    let mut graph = ModuleGraph::new();
    let text = r#"
struct Bucket {
    id: i64
}

fn main() {
    let even = Bucket { id: 0 };
    let odd = Bucket { id: 1 };
    let groups = [1, 2, 3, 4].group_by(|value| if value % 2 == 0 { even } else { odd });
    return groups[even][1];
}
"#;
    let module = graph.add_source(source(1, "main", text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [even] = bindings.locals_named("even") else {
        panic!("expected one even binding");
    };
    let [odd] = bindings.locals_named("odd") else {
        panic!("expected one odd binding");
    };

    let even_let_start = text.find("let even").expect("even let");
    let odd_let_start = text.find("let odd").expect("odd let");
    let even_capture_start = text.find("{ even }").expect("even capture") + "{ ".len();
    let odd_capture_start = text.find("{ odd }").expect("odd capture") + "{ ".len();
    assert_eq!(
        bindings.local_named_at(
            "even",
            LocalBindingKind::Let,
            Span::new(
                SourceId::new(1),
                even_let_start as u32,
                (even_let_start + "let even = Bucket { id: 0 };".len()) as u32
            )
        ),
        Some(*even)
    );
    assert_eq!(
        bindings.local_named_at(
            "odd",
            LocalBindingKind::Let,
            Span::new(
                SourceId::new(1),
                odd_let_start as u32,
                (odd_let_start + "let odd = Bucket { id: 1 };".len()) as u32
            )
        ),
        Some(*odd)
    );
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                even_capture_start as u32,
                (even_capture_start + "even".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*even))
    );
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                odd_capture_start as u32,
                (odd_capture_start + "odd".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*odd))
    );
}

#[test]
fn binding_resolves_block_expression_outer_capture_at_vm_span() {
    let mut graph = ModuleGraph::new();
    let text = r#"
struct Reward {
    count: i64
}

impl Reward {
    fn score(self) {
        return self.count;
    }
}

fn add(base, bonus) {
    return base + bonus;
}

fn main() {
    let reward: Reward = Reward { count: 5 };
    let block_value: i64 = {
        let local = add(bonus = 3, base = 4);
        local + reward.score()
    };
    return block_value;
}
"#;
    let module = graph.add_source(source(1, "main", text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [reward] = bindings.locals_named("reward") else {
        panic!("expected one reward binding");
    };

    let reward_let_start = text.find("let reward").expect("reward let");
    assert_eq!(
        bindings.local_named_at(
            "reward",
            LocalBindingKind::Let,
            Span::new(
                SourceId::new(1),
                reward_let_start as u32,
                (reward_let_start + "let reward: Reward = Reward { count: 5 };".len()) as u32
            )
        ),
        Some(*reward)
    );
    let reward_capture_start = text.find("reward.score()").expect("reward capture");
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                reward_capture_start as u32,
                (reward_capture_start + "reward".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*reward))
    );
}

#[test]
fn binding_resolves_core_conformance_block_capture_at_vm_span() {
    let mut graph = ModuleGraph::new();
    let core = include_str!("../../../../../tests/fixtures/conformance/core_language.vela");
    let reward_module =
        include_str!("../../../../../tests/fixtures/conformance/reward_module.vela");
    let core_module = graph.add_source(source(1, "conformance::core", core));
    graph.add_source(source(2, "conformance::reward", reward_module));
    graph.resolve_imports();
    let main = graph
        .module(core_module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [reward] = bindings.locals_named("reward") else {
        panic!("expected one reward binding");
    };

    let reward_let_start = core.find("let reward").expect("reward let");
    let reward_let_end =
        reward_let_start + "let reward: Reward = Reward { item: \"gold\", count: 5 };".len();
    let reward_capture_start = core.find("reward.score()").expect("reward capture");
    let lambda_capture_start = core.find("reward.count + 9").expect("lambda capture");
    assert_eq!(
        bindings.local_named_at(
            "reward",
            LocalBindingKind::Let,
            Span::new(
                SourceId::new(1),
                reward_let_start as u32,
                reward_let_end as u32
            )
        ),
        Some(*reward)
    );
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                reward_capture_start as u32,
                (reward_capture_start + "reward".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*reward))
    );
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                lambda_capture_start as u32,
                (lambda_capture_start + "reward".len()) as u32
            )
        ),
        Some(&BindingResolution::Local(*reward))
    );
}

#[test]
fn binding_resolves_calls_inside_multiline_if_condition() {
    let mut graph = ModuleGraph::new();
    let text = r#"
fn expect_i64(value: i64) {
    return value;
}

fn expect_i8(value: i8) {
    return value;
}

fn main() {
    let default_integer = 12;
    let contextual: i8 = 7;
    let score: i64 = if expect_i64(default_integer) == 12
        && expect_i8(contextual) == 7i8
    {
        19
    } else {
        0
    };
    return score;
}
"#;
    let module = graph.add_source(source(1, "main", text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let expect_i64 = graph
        .module(module)
        .and_then(|module| module.get("expect_i64"))
        .expect("expect_i64 declaration");
    let expect_i8 = graph
        .module(module)
        .and_then(|module| module.get("expect_i8"))
        .expect("expect_i8 declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");

    let expect_i64_start = text.find("expect_i64(default_integer)").expect("i64 call");
    let expect_i8_start = text.find("expect_i8(contextual)").expect("i8 call");
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                expect_i64_start as u32,
                (expect_i64_start + "expect_i64".len()) as u32
            )
        ),
        Some(&BindingResolution::Declaration(expect_i64))
    );
    assert_eq!(
        hir_resolution_for_span(
            &graph,
            bindings,
            Span::new(
                SourceId::new(1),
                expect_i8_start as u32,
                (expect_i8_start + "expect_i8".len()) as u32
            )
        ),
        Some(&BindingResolution::Declaration(expect_i8))
    );
}

#[test]
fn binding_tracks_attributed_for_loop_locals() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(values) {
    let total = 0;
    #[audit]
    for value in values {
        total += value;
    }
    return total;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [value] = bindings.locals_named("value") else {
        panic!("expected one value binding");
    };
    assert_eq!(
        bindings.local(*value).map(|local| local.kind),
        Some(LocalBindingKind::For)
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Local(*value))
    );
}

#[test]
fn binding_tracks_for_pattern_locals() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
enum Reward {
    Grant { amount },
    Skip { amount },
}
fn main(rewards) {
    let total = 0;
    for Reward::Grant { amount } in rewards {
        total += amount;
    }
    return total;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let amount_bindings = bindings.locals_named("amount");
    assert_eq!(amount_bindings.len(), 1);
    assert_eq!(
        bindings.local(amount_bindings[0]).map(|local| local.kind),
        Some(LocalBindingKind::For)
    );
}

#[test]
fn body_hir_tracks_function_parameters_statements_and_defaults() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
const BASE = 10
fn grant(amount = BASE, bonus = amount + 1) {
    let total = amount + bonus;
    return total;
}
"#,
    ));
    let grant = graph
        .module(module)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let bindings = graph.bindings(grant).expect("grant bindings");
    let [total] = bindings.locals_named("total") else {
        panic!("expected total local");
    };
    let body = graph.function_body(grant).expect("grant body");
    assert_eq!(bindings.body(), body.id);
    assert_eq!(body.owner, HirBodyOwner::Declaration(grant));
    assert!(matches!(body.root, HirBodyRoot::Block(_)));
    assert_eq!(body.params.len(), 2);
    assert!(body.params.iter().all(|param| param.default_body.is_some()));
    assert_eq!(body.statements.len(), 2);
    assert!(body.expressions.len() >= 4);
    let let_statement = body
        .statements
        .values()
        .find(|statement| statement.kind == HirStmtKind::Let)
        .expect("let statement");
    assert!(let_statement.patterns.iter().any(|pattern| {
        body.patterns
            .get(pattern)
            .is_some_and(|pattern| pattern.local == Some(*total))
    }));

    for default_body in body.params.iter().filter_map(|param| param.default_body) {
        let default_body = graph.body(default_body).expect("default body");
        assert!(matches!(
            default_body.owner,
            HirBodyOwner::ParameterDefault { parent, .. } if parent == body.id
        ));
        assert!(matches!(default_body.root, HirBodyRoot::Expr(_)));
        assert!(!default_body.expressions.is_empty());
    }
}

#[test]
fn body_hir_tracks_lambda_bodies_and_captures() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(player) {
    let base = player.level;
    let mapper = |amount| amount + base;
    return mapper;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [base] = bindings.locals_named("base") else {
        panic!("expected base local");
    };

    let function_body = graph.function_body(main).expect("main body");
    let lambda_body = graph
        .bodies()
        .find(|body| matches!(body.owner, HirBodyOwner::Lambda { parent, .. } if parent == function_body.id))
        .expect("lambda body");
    assert_eq!(lambda_body.params.len(), 1);
    assert!(matches!(lambda_body.root, HirBodyRoot::Expr(_)));
    assert!(
        lambda_body
            .captures
            .iter()
            .any(|capture| capture.local == *base)
    );
    assert!(lambda_body.root_scope.is_some());
    assert!(lambda_body.scopes.values().any(|scope| {
        scope.kind == HirScopeKind::Body
            && scope
                .locals
                .iter()
                .any(|local| lambda_body.params.iter().any(|param| param.local == *local))
    }));
}

#[test]
fn body_hir_tracks_transitive_lambda_captures() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(base) {
    let outer = |amount| {
        return |bonus| base + amount + bonus;
    };
    return outer;
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [base] = bindings.locals_named("base") else {
        panic!("expected base local");
    };
    let [amount] = bindings.locals_named("amount") else {
        panic!("expected amount local");
    };

    let function_body = graph.function_body(main).expect("main body");
    let outer_body = graph
        .bodies()
        .find(|body| matches!(body.owner, HirBodyOwner::Lambda { parent, .. } if parent == function_body.id))
        .expect("outer lambda body");
    let inner_body = graph
        .bodies()
        .find(|body| matches!(body.owner, HirBodyOwner::Lambda { parent, .. } if parent == outer_body.id))
        .expect("inner lambda body");

    assert!(
        outer_body
            .captures
            .iter()
            .any(|capture| capture.local == *base),
        "outer lambda should capture base for the nested lambda"
    );
    assert!(
        inner_body
            .captures
            .iter()
            .any(|capture| capture.local == *base),
        "inner lambda should capture base"
    );
    assert!(
        inner_body
            .captures
            .iter()
            .any(|capture| capture.local == *amount),
        "inner lambda should capture amount"
    );
}

#[test]
fn body_hir_tracks_lexical_scopes_for_blocks_loops_and_match_arms() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(values, state) {
    let total = 0;
    if total == 0 {
        let branch = total;
    }
    for (index, value) in values {
        let nested = value;
    }
    match state {
        Reward::Grant { amount } => amount,
        _ => total,
    }
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let body = graph.function_body(main).expect("main body");

    assert!(body.root_scope.is_some());
    assert!(
        body.scopes
            .values()
            .any(|scope| scope.kind == HirScopeKind::For)
    );
    assert!(
        body.scopes
            .values()
            .any(|scope| scope.kind == HirScopeKind::Block)
    );
    assert!(
        body.scopes
            .values()
            .filter(|scope| scope.kind == HirScopeKind::MatchArm)
            .count()
            >= 2
    );

    let [amount] = bindings.locals_named("amount") else {
        panic!("expected amount pattern local");
    };
    assert!(body.scopes.values().any(|scope| {
        scope.kind == HirScopeKind::MatchArm && scope.locals.iter().any(|local| local == amount)
    }));
}

#[test]
fn tuple_pattern_bindings_use_binding_token_spans() {
    let text = r#"
fn main(pairs) {
    let (first_name, score_value) = ("Ada", 1);
    for (loop_name, loop_score) in pairs {
        loop_score;
    }
    match ("Grace", 2) {
        (match_name, match_score) => match_name
    }
}
"#;
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(1, "game::reward", text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");

    for (name, kind) in [
        ("first_name", LocalBindingKind::Let),
        ("score_value", LocalBindingKind::Let),
        ("loop_name", LocalBindingKind::For),
        ("loop_score", LocalBindingKind::For),
        ("match_name", LocalBindingKind::Pattern),
        ("match_score", LocalBindingKind::Pattern),
    ] {
        let [local] = bindings.locals_named(name) else {
            panic!("expected one {name} binding");
        };
        let start = text.find(name).expect("binding token should exist");
        assert_eq!(
            bindings.local(*local).map(|local| (local.kind, local.span)),
            Some((
                kind,
                Span::new(SourceId::new(1), start as u32, (start + name.len()) as u32)
            )),
            "{name} binding should use token span"
        );
    }
}

#[test]
fn binding_reports_unresolved_match_arm_body_paths() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(value) {
    match value {
        _ => missing_symbol
    }
}
"#,
    ));

    assert!(
        graph
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `missing_symbol`"),
        "{:?}",
        graph.diagnostics()
    );
}

#[test]
fn body_hir_records_unresolved_references() {
    let text = r#"
fn main(value) {
    let total = value + missing_symbol;
    missing_call(total);
    return total;
}
"#;
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(1, "game::reward", text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let body = graph.function_body(main).expect("main body");
    let missing_start = text.find("missing_symbol").expect("missing symbol");
    let missing_span = Span::new(
        SourceId::new(1),
        missing_start as u32,
        (missing_start + "missing_symbol".len()) as u32,
    );
    let missing_call_start = text.find("missing_call").expect("missing call");
    let missing_call_span = Span::new(
        SourceId::new(1),
        missing_call_start as u32,
        (missing_call_start + "missing_call".len()) as u32,
    );

    assert!(
        graph
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `missing_symbol`"),
        "{:?}",
        graph.diagnostics()
    );
    assert!(
        graph
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.message != "unresolved name `missing_call`"),
        "{:?}",
        graph.diagnostics()
    );
    assert_eq!(body.unresolved_references.len(), 2);
    for (name, span) in [
        ("missing_symbol", missing_span),
        ("missing_call", missing_call_span),
    ] {
        let unresolved = body
            .unresolved_references
            .iter()
            .find(|reference| reference.name == name)
            .expect("unresolved reference");
        assert_eq!(unresolved.origin.span, span);
        assert!(body.expressions.contains_key(&unresolved.expression));
        assert_eq!(graph.expression_at_span(span), Some(unresolved.expression));
    }
}

#[test]
fn body_hir_resolves_expression_containing_source_spans() {
    let text = r#"
enum Reward { Grant, Skip }
fn main() {
    return Reward::Grant;
}
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(source(1, "game::reward", text));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let variant_start = text.find("Grant;").expect("variant segment");
    let variant_span = Span::new(
        SourceId::new(1),
        variant_start as u32,
        (variant_start + "Grant".len()) as u32,
    );
    let expression = graph
        .expression_containing_span(variant_span)
        .expect("qualified path expression");

    let path_start = text.find("Reward::Grant").expect("qualified path");
    let path_span = Span::new(
        SourceId::new(1),
        path_start as u32,
        (path_start + "Reward::Grant".len()) as u32,
    );
    assert_eq!(graph.expression_at_span(path_span), Some(expression));
    assert_eq!(graph.expression_span(expression), Some(path_span));
}

#[test]
fn duplicate_lambda_parameters_report_both_spans() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main(reward) {
    let mapper = |count, count| count;
    return mapper(reward);
}
"#,
    ));
    let duplicate = graph
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_parameter"))
        .expect("duplicate lambda parameter diagnostic");
    assert_eq!(duplicate.labels.len(), 2);
    assert!(duplicate.labels[0].message.contains("previous"));
    assert!(duplicate.labels[1].message.contains("duplicate"));
    assert_ne!(duplicate.labels[0].span, duplicate.labels[1].span);
}
#[test]
fn function_bindings_resolve_imported_names() {
    let mut graph = ModuleGraph::new();
    let reward = graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    let module = graph.add_source(source(
        2,
        "game::main",
        r#"
use game::reward::grant
fn main() { return grant; }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
    );
}
#[test]
fn function_bindings_resolve_import_aliases() {
    let mut graph = ModuleGraph::new();
    let reward = graph.add_source(source(1, "game::reward", "pub fn grant() { return 1; }"));
    let module = graph.add_source(source(
        2,
        "game::main",
        r#"
use game::reward::grant as give_reward
fn main() { return give_reward; }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    let imports = graph.imports(module).expect("module imports");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert_eq!(imports[0].alias.as_deref(), Some("give_reward"));
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
    );
}
#[test]
fn function_bindings_resolve_record_constructor_import_aliases() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::reward::Reward as Prize
fn main() {
    return Prize { count: 2 };
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game::reward",
        r#"
pub struct Reward { count: i64 }
"#,
    ));
    graph.resolve_imports();
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let reward = graph
        .module(reward)
        .and_then(|module| module.get("Reward"))
        .expect("reward declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(reward) })
    );
}
#[test]
fn function_bindings_resolve_match_pattern_import_aliases() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::damage::Damage as Hit
fn main(damage) {
    match damage {
        Hit::Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
    ));
    let damage = graph.add_source(source(
        2,
        "game::damage",
        r#"
pub enum Damage { Physical }
"#,
    ));
    graph.resolve_imports();
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let damage = graph
        .module(damage)
        .and_then(|module| module.get("Damage"))
        .expect("damage declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(bindings.pattern_resolutions().any(|(path, resolution)| {
        path == ["Hit".to_owned(), "Physical".to_owned()]
            && resolution == &BindingResolution::Declaration(damage)
    }));
}
#[test]
fn function_bindings_resolve_tuple_constructor_call_aliases() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::damage::Damage as Hit
fn main() {
    return Hit::Physical(7);
}
"#,
    ));
    let damage = graph.add_source(source(
        2,
        "game::damage",
        r#"
pub enum Damage { Physical(amount) }
"#,
    ));
    graph.resolve_imports();
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let damage = graph
        .module(damage)
        .and_then(|module| module.get("Damage"))
        .expect("damage declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(damage) })
    );
}
#[test]
fn resolved_imports_refresh_existing_binding_maps() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::reward::grant
fn main() { return grant; }
"#,
    ));
    let reward = graph.add_source(source(2, "game::reward", "pub fn grant() { return 1; }"));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    assert!(
        graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| {
                resolution == &BindingResolution::Import("grant".to_owned())
            })
    );
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    assert!(
        graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
    );
    assert!(
        !graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| {
                resolution == &BindingResolution::Import("grant".to_owned())
            })
    );
}
#[test]
fn resolved_modules_refresh_qualified_path_binding_maps() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
fn main() {
    return game::reward::grant() + game::config::BONUS;
}
"#,
    ));
    let reward = graph.add_source(source(
        2,
        "game::reward",
        r#"
pub fn grant() { return 4; }
"#,
    ));
    let config = graph.add_source(source(
        3,
        "game::config",
        r#"
pub const BONUS: i64 = 5;
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let grant = graph
        .module(reward)
        .and_then(|module| module.get("grant"))
        .expect("grant declaration");
    let bonus = graph
        .module(config)
        .and_then(|module| module.get("BONUS"))
        .expect("bonus declaration");
    assert!(
        graph
            .bindings(main)
            .expect("main bindings")
            .resolutions()
            .any(|(_, resolution)| {
                resolution
                    == &BindingResolution::QualifiedPath(vec![
                        "game".to_owned(),
                        "reward".to_owned(),
                        "grant".to_owned(),
                    ])
            })
    );
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Declaration(grant))
    );
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| resolution == &BindingResolution::Declaration(bonus))
    );
}
#[test]
fn qualified_private_paths_do_not_resolve_across_modules() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
fn main() {
    return game::reward::secret();
}
"#,
    ));
    graph.add_source(source(
        2,
        "game::reward",
        r#"
fn secret() { return 1; }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    assert!(bindings.resolutions().any(|(_, resolution)| {
        resolution
            == &BindingResolution::QualifiedPath(vec![
                "game".to_owned(),
                "reward".to_owned(),
                "secret".to_owned(),
            ])
    }));
}
#[test]
fn binding_treats_bare_map_keys_as_keys_not_name_reads() {
    let mut graph = ModuleGraph::new();
    graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main() {
    return { exp: 15 };
}
"#,
    ));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
}
#[test]
fn binding_resolves_record_shorthand_fields() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::reward",
        r#"
fn main() {
    let count = 2;
    return Reward { count };
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let bindings = graph.bindings(main).expect("main bindings");
    let [count] = bindings.locals_named("count") else {
        panic!("expected count binding");
    };
    assert!(
        bindings
            .resolutions()
            .any(|(_, resolution)| { resolution == &BindingResolution::Local(*count) })
    );
}
