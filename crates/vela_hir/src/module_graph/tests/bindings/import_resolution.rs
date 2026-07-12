use super::*;

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
fn body_hir_owns_executable_operands_containers_and_control_flow() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
enum State { Ready { count } }
fn main(player, values) {
    let pair = (1, 2);
    let data = { key: pair.0 };
    let bump = |x| x + 1;
    player.level += data["key"];
    for value in values {
        if value > 0 { player.save(value); }
    }
    match player.state {
        State::Ready { count } if count > 0 => bump(count),
        _ => 0,
    }
}
"#,
    ));
    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let body = graph.function_body(main).expect("main body");
    assert!(
        body.expressions
            .values()
            .all(|expression| { !matches!(&expression.kind, HirExprKind::Missing) })
    );
    assert!(body.expressions.values().any(|expression| {
        matches!(&expression.kind, HirExprKind::Tuple { elements } if elements.len() == 2)
    }));
    assert!(body.expressions.values().any(|expression| {
        matches!(&expression.kind, HirExprKind::Map { entries }
            if entries.len() == 1 && entries[0].key.is_some() && entries[0].value.is_some())
    }));
    assert!(body.expressions.values().any(|expression| {
        matches!(&expression.kind, HirExprKind::Assign {
            target: Some(target), value: Some(value), ..
        } if body.expressions.contains_key(target) && body.expressions.contains_key(value))
    }));
    assert!(body.expressions.values().any(|expression| {
        matches!(&expression.kind, HirExprKind::Call(call)
            if body.expressions.contains_key(&call.callee)
                && call.arguments.iter().all(|argument| argument.value
                    .is_none_or(|value| body.expressions.contains_key(&value))))
    }));
    assert!(body.statements.values().any(|statement| {
        matches!(&statement.kind, HirStmtKind::For {
            iterable: Some(iterable), body: Some(block), ..
        } if body.expressions.contains_key(iterable) && body.blocks.contains_key(block))
    }));
    let match_expr = body
        .statements
        .values()
        .find_map(|statement| {
            let HirStmtKind::Match(value) = &statement.kind else {
                return None;
            };
            Some(value)
        })
        .expect("match expression");
    assert_eq!(match_expr.arms.len(), 2);
    assert!(match_expr.arms.iter().all(|arm| {
        body.match_arms.get(arm).is_some_and(|arm| {
            arm.pattern.is_some() && arm.body.is_some() && body.scopes.contains_key(&arm.scope)
        })
    }));

    let lambda = body
        .expressions
        .values()
        .find_map(|expression| {
            let HirExprKind::Lambda { body } = expression.kind else {
                return None;
            };
            Some(body)
        })
        .expect("lambda body id");
    let lambda = graph.body(lambda).expect("lambda body");
    assert!(matches!(lambda.root, HirBodyRoot::Expr(_)));
    assert!(lambda.expressions.values().any(|expression| {
        matches!(&expression.kind, HirExprKind::Binary {
            lhs: Some(lhs), rhs: Some(rhs), ..
        } if lambda.expressions.contains_key(lhs) && lambda.expressions.contains_key(rhs))
    }));
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
fn constructor_bindings_resolve_after_later_qualified_sources_are_added() {
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(source(
        1,
        "game::main",
        r#"
use game::schema::Reward as Prize
use game::schema::State as ImportedState

fn main(value) {
    Prize { amount: 1 };
    game::schema::Reward { amount: 2 };
    ImportedState::Ready { amount: 3 };
    game::schema::State::Ready { amount: 4 };
    Missing { amount: 5 };
    match value {
        ImportedState::Ready { amount } => {},
        game::schema::State::Idle => {},
        Missing::Ready { amount } => {},
    }
}
"#,
    ));
    let schema = graph.add_source(source(
        2,
        "game::schema",
        r#"
pub struct Reward { amount: i64 }
pub enum State { Ready { amount: i64 }, Idle }
"#,
    ));
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());

    let main = graph
        .module(module)
        .and_then(|module| module.get("main"))
        .expect("main declaration");
    let reward = graph
        .module(schema)
        .and_then(|module| module.get("Reward"))
        .expect("Reward declaration");
    let state = graph
        .module(schema)
        .and_then(|module| module.get("State"))
        .expect("State declaration");
    let body = graph.function_body(main).expect("main body");
    let bindings = graph.bindings(main).expect("main bindings");

    for path in body.paths.iter() {
        match (path.kind, path.owner) {
            (HirPathKind::Constructor, HirPathOwner::Expression(expression)) => {
                let expected = match path.path.join("::").as_str() {
                    "Prize" | "game::schema::Reward" => ConstructorResolution::Declaration(reward),
                    "ImportedState::Ready" | "game::schema::State::Ready" => {
                        ConstructorResolution::Declaration(state)
                    }
                    "Missing" => ConstructorResolution::Dynamic(path.path.clone()),
                    other => panic!("unexpected constructor path `{other}`"),
                };
                assert_eq!(bindings.constructor_resolution(expression), Some(expected));
            }
            (HirPathKind::Pattern, HirPathOwner::Pattern(_)) => {
                let expected = match path.path.join("::").as_str() {
                    "ImportedState::Ready" | "game::schema::State::Idle" => {
                        ConstructorResolution::Declaration(state)
                    }
                    "Missing::Ready" => ConstructorResolution::Dynamic(path.path.clone()),
                    other => panic!("unexpected pattern path `{other}`"),
                };
                assert_eq!(
                    bindings.pattern_constructor_resolution(&path.path),
                    Some(expected)
                );
            }
            _ => {}
        }
    }
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
