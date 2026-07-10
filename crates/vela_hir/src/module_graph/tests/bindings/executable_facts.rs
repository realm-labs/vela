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
fn function_bodies_record_call_callee_expression_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
fn helper() { return 1; }
fn main() { return helper(); }
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let helper = graph
        .module(module)
        .and_then(|module| module.get("helper"))
        .expect("helper declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let body = graph.function_body(main).expect("main body");
    let call = body
        .calls()
        .map(|(_, call)| call)
        .find(|call| {
            body.expressions
                .get(&call.expression)
                .is_some_and(|expression| matches!(&expression.kind, HirExprKind::Call(_)))
        })
        .expect("call record");
    let callee = graph
        .call_callee(call.expression)
        .expect("callee expression id");
    assert_eq!(callee, call.callee);
    let bindings = graph.bindings(main).expect("main bindings");
    assert_eq!(
        bindings.resolution(callee),
        Some(&BindingResolution::Declaration(helper))
    );
}

#[test]
fn function_bodies_record_member_call_field_callees() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
fn main(player) {
    player.grant();
}
"#;
    graph.add_source(source(1, "game::main", source_text));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let calls = graph
        .member_calls_in_source(SourceId::new(1))
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(calls, vec!["grant"]);
}

#[test]
fn function_bodies_record_field_receiver_expression_ids() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
fn main(player) {
    return player.level;
}
"#;
    let module = graph.add_source(source(1, "game::main", source_text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let member_start = source_text.find("level").expect("member token") as u32;
    let member_span = Span::new(
        SourceId::new(1),
        member_start,
        member_start + "level".len() as u32,
    );
    let field = graph
        .field_at_member_span(member_span)
        .expect("field member fact");
    assert_eq!(field.name, "level");
    assert_eq!(
        graph
            .fields_in_source(SourceId::new(1))
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["level"]
    );
    let body = graph.function_body(main).expect("main body");
    assert!(
        body.expressions
            .get(&field.expression)
            .is_some_and(|expression| matches!(&expression.kind, HirExprKind::Field(_)))
    );
    assert_eq!(
        graph.expression_span(field.receiver),
        Some(Span::new(
            SourceId::new(1),
            source_text.find("player.level").expect("receiver") as u32,
            (source_text.find("player.level").expect("receiver") + "player".len()) as u32,
        ))
    );
}

#[test]
fn function_bodies_record_index_operand_expression_ids() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
fn main(values, key) {
    return values[key];
}
"#;
    let module = graph.add_source(source(1, "game::main", source_text));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let body = graph.function_body(main).expect("main body");
    let index = body
        .expressions
        .values()
        .filter_map(|expression| match &expression.kind {
            HirExprKind::Index(index) => Some(index),
            _ => None,
        })
        .find(|index| {
            body.expressions
                .get(&index.expression)
                .is_some_and(|expression| matches!(&expression.kind, HirExprKind::Index(_)))
        })
        .expect("index fact");
    assert_eq!(
        graph.index_for_expression(index.expression),
        Some(index),
        "module graph should expose the HIR index fact"
    );
    assert_eq!(
        graph.expression_span(index.receiver),
        Some(Span::new(
            SourceId::new(1),
            source_text.find("values[key]").expect("receiver") as u32,
            (source_text.find("values[key]").expect("receiver") + "values".len()) as u32,
        ))
    );
    assert_eq!(
        graph.expression_span(index.index),
        Some(Span::new(
            SourceId::new(1),
            source_text.find("key]").expect("index") as u32,
            (source_text.find("key]").expect("index") + "key".len()) as u32,
        ))
    );
}

#[test]
fn function_bodies_record_tuple_projection_field_facts() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
fn main(pair) {
    return pair.0;
}
"#;
    graph.add_source(source(1, "game::main", source_text));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let member_start = source_text.find('0').expect("tuple projection member") as u32;
    let member_span = Span::new(SourceId::new(1), member_start, member_start + 1);
    let field = graph
        .field_at_member_span(member_span)
        .expect("tuple projection field fact");
    assert_eq!(field.name, "0");
    assert_eq!(
        graph
            .fields_in_source(SourceId::new(1))
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["0"]
    );
    assert_eq!(
        graph.expression_span(field.receiver),
        Some(Span::new(
            SourceId::new(1),
            source_text.find("pair.0").expect("receiver") as u32,
            (source_text.find("pair.0").expect("receiver") + "pair".len()) as u32,
        ))
    );
}

#[test]
fn function_bodies_record_interpolated_string_path_facts() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
fn main(name, amount) {
    return f"reward {{ready}} {name}: {amount}\n";
}
"#;
    let module = graph.add_source(source(1, "game::main", source_text));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let body = graph.function_body(main).expect("main body");
    let parts = body
        .expressions
        .values()
        .find_map(|expression| match &expression.kind {
            HirExprKind::Literal(HirLiteral::Interpolated { parts }) => Some(parts),
            _ => None,
        })
        .expect("interpolated literal parts");
    assert_eq!(parts.len(), 5);
    assert_eq!(
        parts[0],
        HirInterpolatedStringPart::Text("reward {ready} ".to_owned())
    );
    assert!(matches!(parts[1], HirInterpolatedStringPart::Expr(_)));
    assert_eq!(parts[2], HirInterpolatedStringPart::Text(": ".to_owned()));
    assert!(matches!(parts[3], HirInterpolatedStringPart::Expr(_)));
    assert_eq!(parts[4], HirInterpolatedStringPart::Text("\n".to_owned()));

    let path_facts = graph
        .paths_in_source_by_kind(SourceId::new(1), HirPathKind::Value)
        .map(|path| {
            (
                path.path.clone(),
                path.origin.span.start,
                path.origin.span.end,
            )
        })
        .collect::<Vec<_>>();

    assert!(path_facts.iter().any(|(path, start, end)| {
        path == &["name"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("name")
    }));
    assert!(path_facts.iter().any(|(path, start, end)| {
        path == &["amount"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("amount")
    }));
}

#[test]
fn schema_field_defaults_record_value_path_facts() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
const BASE_COUNT: i64 = 2

struct Reward {
    count: i64 = BASE_COUNT + 3,
}
"#;
    graph.add_source(source(1, "game::main", source_text));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let path_facts = graph
        .paths_in_source_by_kind(SourceId::new(1), HirPathKind::Value)
        .map(|path| {
            (
                path.path.clone(),
                path.origin.span.start,
                path.origin.span.end,
            )
        })
        .collect::<Vec<_>>();

    assert!(path_facts.iter().any(|(path, start, end)| {
        path == &["BASE_COUNT"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("BASE_COUNT")
    }));
}

#[test]
fn function_bodies_record_source_path_facts() {
    let mut graph = ModuleGraph::new();
    let source_text = r#"
enum QuestState {
    Active { count: i64 }
    Done
}
fn grant_reward() { return 1; }
fn main(states) {
    grant_reward()
    let weights = { RewardKey::Small: 1 }
    let next = QuestState::Active { count: 1 }
    match next {
        QuestState::Active { count } => count
        QuestState::Done => 0
    }
}
"#;
    graph.add_source(source(1, "game::main", source_text));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let path_facts = graph
        .paths_in_source(SourceId::new(1))
        .map(|path| {
            (
                path.kind,
                path.path.clone(),
                path.segment_origin.span.start,
                path.segment_origin.span.end,
            )
        })
        .collect::<Vec<_>>();

    assert!(path_facts.iter().any(|(kind, path, start, end)| {
        *kind == HirPathKind::Callee
            && path == &["grant_reward"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("grant_reward")
    }));
    assert!(path_facts.iter().any(|(kind, path, start, end)| {
        *kind == HirPathKind::Constructor
            && path == &["QuestState", "Active"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("Active")
    }));
    assert!(path_facts.iter().any(|(kind, path, start, end)| {
        *kind == HirPathKind::Value
            && path == &["RewardKey", "Small"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("Small")
    }));
    assert!(path_facts.iter().any(|(kind, path, start, end)| {
        *kind == HirPathKind::Pattern
            && path == &["QuestState", "Done"]
            && source_text.get(
                usize::try_from(*start).expect("path start fits usize")
                    ..usize::try_from(*end).expect("path end fits usize"),
            ) == Some("Done")
    }));
}

#[test]
fn let_statements_record_initializer_expression_ids() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
struct Reward { count: i64 }
fn main() {
    let reward = Reward { count: 2 }
    return reward
}
"#,
    ));
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let body = graph.function_body(main).expect("main body");
    let statement = body
        .statements
        .values()
        .find(|statement| statement.tag() == HirStmtTag::Let)
        .expect("let statement");
    let initializer = statement.initializer().expect("let initializer expression");
    assert!(body.expressions.contains_key(&initializer));
    assert!(body.paths.iter().any(|path| {
        path.kind == HirPathKind::Constructor
            && path.owner == HirPathOwner::Expression(initializer)
            && path.path.iter().map(String::as_str).eq(["Reward"])
    }));
    assert!(statement.patterns().iter().any(|pattern| {
        body.patterns
            .get(pattern)
            .is_some_and(|pattern| pattern.local().is_some())
    }));
}

include!("import_resolution.rs");
