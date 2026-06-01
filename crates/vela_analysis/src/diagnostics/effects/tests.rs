use vela_common::{FunctionId, HostMethodId, SourceId, TypeId};
use vela_reflect::access::{FunctionEffectSet, MethodEffectSet};
use vela_reflect::modules::FunctionDesc;
use vela_reflect::registry::{MethodDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_syntax::ast::{Expr, ItemKind, StmtKind};
use vela_syntax::parser::parse_source;

use super::*;

#[test]
fn reports_disallowed_function_and_method_effects() {
    let exprs = function_exprs(
        r#"
        fn main(player) {
            grant_reward(player);
            player.grant_exp(5);
            pure_score(player);
        }
        "#,
    );
    let scope = ExprFactScope::new().with_path(["player"], TypeFact::host("Player"));
    let facts = effect_registry_facts();

    let function_diagnostics =
        effect_diagnostics(&exprs[0], &scope, &facts, &RegistryEffectFact::pure());
    let method_diagnostics =
        effect_diagnostics(&exprs[1], &scope, &facts, &RegistryEffectFact::pure());
    let pure_diagnostics =
        effect_diagnostics(&exprs[2], &scope, &facts, &RegistryEffectFact::pure());

    assert_eq!(function_diagnostics.len(), 1);
    assert_eq!(
        function_diagnostics[0].code.as_deref(),
        Some("analysis::disallowed_effect")
    );
    assert!(function_diagnostics[0].message.contains("emits_events"));
    assert_eq!(method_diagnostics.len(), 1);
    assert!(method_diagnostics[0].message.contains("writes_host"));
    assert!(pure_diagnostics.is_empty());
}

#[test]
fn allowed_effects_do_not_report_diagnostics() {
    let exprs = function_exprs(
        r#"
        fn main(player) {
            grant_reward(player);
            player.grant_exp(5);
        }
        "#,
    );
    let scope = ExprFactScope::new().with_path(["player"], TypeFact::host("Player"));
    let facts = effect_registry_facts();
    let allowed = RegistryEffectFact {
        reads_host: true,
        writes_host: true,
        emits_events: true,
    };

    assert!(effect_diagnostics(&exprs[0], &scope, &facts, &allowed).is_empty());
    assert!(effect_diagnostics(&exprs[1], &scope, &facts, &allowed).is_empty());
}

#[test]
fn unknown_call_effects_degrade_without_diagnostics() {
    let exprs = function_exprs(
        r#"
        fn main(dynamic_value) {
            missing_call();
            dynamic_value.unknown_method();
        }
        "#,
    );
    let scope = ExprFactScope::new();
    let facts = effect_registry_facts();

    assert!(effect_diagnostics(&exprs[0], &scope, &facts, &RegistryEffectFact::pure()).is_empty());
    assert!(effect_diagnostics(&exprs[1], &scope, &facts, &RegistryEffectFact::pure()).is_empty());
}

fn effect_registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player")).method(
            MethodDesc::new(HostMethodId::new(1), "grant_exp")
                .effects(MethodEffectSet::host_write()),
        ),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "grant_reward")
            .effects(FunctionEffectSet::event_emit()),
    );
    registry.register_function(FunctionDesc::new(FunctionId::new(2), "pure_score"));
    RegistryFacts::from_registry(&registry)
}

fn function_exprs(source: &str) -> Vec<Expr> {
    let parsed = parse_source(SourceId::new(1), source);
    assert_eq!(parsed.diagnostics, []);
    let function = parsed
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) => Some(function),
            _ => None,
        })
        .expect("function item");

    function
        .body
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            StmtKind::Expr(expr) => Some(expr.clone()),
            StmtKind::Let {
                value: Some(expr), ..
            } => Some(expr.clone()),
            _ => None,
        })
        .collect()
}
