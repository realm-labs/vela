use vela_common::{SourceId, TypeId, VariantId};
use vela_reflect::registry::{TypeDesc, TypeKey, TypeRegistry, VariantDesc};
use vela_syntax::ast::{Expr, ItemKind, StmtKind};
use vela_syntax::parser::parse_source;

use super::*;

#[test]
fn reports_unknown_variants_for_known_enum_match_patterns() {
    let exprs = function_exprs(
        r#"
        fn main(quest) {
            match quest {
                QuestState.Activ => 1,
                QuestState.Finished => 0,
            };
        }
        "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
    let facts = enum_registry_facts();

    let diagnostics = match_pattern_diagnostics(&exprs[0], &scope, &facts);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("analysis::unknown_variant")
    );
    assert!(diagnostics[0].message.contains("Activ"));
    assert!(
        diagnostics[0]
            .labels
            .iter()
            .any(|label| label.message.contains("Active"))
    );
}

#[test]
fn reports_unknown_dynamic_option_result_variants() {
    let exprs = function_exprs(
        r#"
        fn main(maybe, result) {
            match maybe {
                Option.Smoe(value) => value,
                Option.None => 0,
            };
            match result {
                Result.Okk(value) => value,
                Result.Err(error) => error,
            };
        }
        "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["maybe"], TypeFact::option(TypeFact::Int))
        .with_path(
            ["result"],
            TypeFact::result(TypeFact::Int, TypeFact::String),
        );
    let facts = RegistryFacts::default();

    let option_diagnostics = match_pattern_diagnostics(&exprs[0], &scope, &facts);
    let result_diagnostics = match_pattern_diagnostics(&exprs[1], &scope, &facts);

    assert_eq!(option_diagnostics.len(), 1);
    assert!(
        option_diagnostics[0]
            .labels
            .iter()
            .any(|label| label.message.contains("Some"))
    );
    assert_eq!(result_diagnostics.len(), 1);
    assert!(
        result_diagnostics[0]
            .labels
            .iter()
            .any(|label| label.message.contains("Ok"))
    );
}

#[test]
fn skips_dynamic_or_different_owner_patterns() {
    let exprs = function_exprs(
        r#"
        fn main(quest, unknown) {
            match quest {
                Other.Activ => 1,
                QuestState.Active => 0,
            };
            match unknown {
                Missing.Active => 1,
            };
        }
        "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
    let facts = enum_registry_facts();

    assert!(match_pattern_diagnostics(&exprs[0], &scope, &facts).is_empty());
    assert!(match_pattern_diagnostics(&exprs[1], &scope, &facts).is_empty());
}

fn enum_registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestState"))
            .kind(vela_reflect::registry::TypeKind::ScriptEnum)
            .variant(VariantDesc::new(VariantId::new(1), "Active"))
            .variant(VariantDesc::new(VariantId::new(2), "Finished")),
    );
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
