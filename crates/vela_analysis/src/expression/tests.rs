use vela_common::SourceId;
use vela_def::{FieldId, TypeId, VariantId};
use vela_reflect::registry::{
    FieldDesc, HostIndexCapability, TypeDesc, TypeKey, TypeKind, TypeRegistry, VariantDesc,
};
use vela_syntax::ast::{ItemKind, StmtKind};
use vela_syntax::parser::parse_source;

use super::*;

#[test]
fn infers_literal_array_map_and_record_facts() {
    let expressions = function_exprs(
        r#"
            struct Reward { count: i64 }
            fn main() {
                let values = [1, 2, 3];
                let rewards = {"quest": 1, boss: 2.5, 10: 3};
                let reward = Reward { count: 3 };
            }
            "#,
    );
    let scope = ExprFactScope::new();

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::array(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::map(
            TypeFact::STRING,
            TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
        )
    );
    assert_eq!(
        type_fact_from_expr(&expressions[2], &scope),
        TypeFact::record("Reward")
    );
}

#[test]
fn infers_path_and_branch_facts_from_scope() {
    let expressions = function_exprs(
        r#"
            fn main() {
                if ok { score } else { "none" };
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["ok"], TypeFact::BOOL)
        .with_path(["score"], TypeFact::I64);

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::Union(vec![TypeFact::I64, TypeFact::STRING])
    );
}

#[test]
fn infers_index_read_facts_from_collection_receivers() {
    let expressions = function_exprs(
        r#"
            fn main() {
                scores[0];
                rewards["gold"];
                either[0];
                scores["bad"];
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["scores"], TypeFact::array(TypeFact::I64))
        .with_path(["rewards"], TypeFact::map(TypeFact::STRING, TypeFact::F64))
        .with_path(
            ["either"],
            TypeFact::union([
                TypeFact::array(TypeFact::I64),
                TypeFact::map(TypeFact::STRING, TypeFact::F64),
            ]),
        );

    assert_eq!(type_fact_from_expr(&expressions[0], &scope), TypeFact::I64);
    assert_eq!(type_fact_from_expr(&expressions[1], &scope), TypeFact::F64);
    assert_eq!(type_fact_from_expr(&expressions[2], &scope), TypeFact::I64);
    assert_eq!(
        type_fact_from_expr(&expressions[3], &scope),
        TypeFact::Unknown
    );
}

#[test]
fn infers_index_read_facts_from_host_index_capability() {
    let expressions = function_exprs(
        r#"
            fn main() {
                scores[0];
                scores["bad"];
            }
            "#,
    );
    let scope = ExprFactScope::new().with_path(["scores"], TypeFact::host("Scores"));
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(99), "Scores")).index_capability(
            HostIndexCapability::new()
                .readable(true)
                .key_type("i64")
                .value_type("string"),
        ),
    );
    let facts = RegistryFacts::from_registry(&registry);

    assert_eq!(
        type_fact_from_expr_with_registry(&expressions[0], &scope, &facts),
        TypeFact::STRING
    );
    assert_eq!(
        type_fact_from_expr_with_registry(&expressions[1], &scope, &facts),
        TypeFact::Unknown
    );
}

#[test]
fn infers_try_propagation_payload_facts() {
    let expressions = function_exprs(
        r#"
            fn main() {
                maybe?;
                some?;
                none?;
                grant?;
                failed?;
                either?;
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["maybe"], TypeFact::option(TypeFact::I64))
        .with_path(["some"], TypeFact::option_some(TypeFact::STRING))
        .with_path(["none"], TypeFact::option_none())
        .with_path(
            ["grant"],
            TypeFact::result(TypeFact::host("Reward"), TypeFact::STRING),
        )
        .with_path(["failed"], TypeFact::result_err(TypeFact::record("Error")))
        .with_path(
            ["either"],
            TypeFact::union([
                TypeFact::option(TypeFact::I64),
                TypeFact::result_ok(TypeFact::STRING),
                TypeFact::option_none(),
            ]),
        );

    assert_eq!(type_fact_from_expr(&expressions[0], &scope), TypeFact::I64);
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::STRING
    );
    assert_eq!(
        type_fact_from_expr(&expressions[2], &scope),
        TypeFact::Never
    );
    assert_eq!(
        type_fact_from_expr(&expressions[3], &scope),
        TypeFact::host("Reward")
    );
    assert_eq!(
        type_fact_from_expr(&expressions[4], &scope),
        TypeFact::Never
    );
    assert_eq!(
        type_fact_from_expr(&expressions[5], &scope),
        TypeFact::union([TypeFact::I64, TypeFact::STRING])
    );
}

#[test]
fn narrows_null_checked_branch_facts() {
    let expressions = function_exprs(
        r#"
            fn main() {
                if player == null { 0 } else { player };
            }
            "#,
    );
    let scope = ExprFactScope::new().with_path(
        ["player"],
        TypeFact::Union(vec![TypeFact::NULL, TypeFact::host("Player")]),
    );

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::Union(vec![TypeFact::I64, TypeFact::host("Player")])
    );
}

#[test]
fn infers_null_fallback_for_if_expression_without_else() {
    let expressions = function_exprs(
        r#"
            fn main() {
                if enabled { 7 };
            }
            "#,
    );

    assert_eq!(
        type_fact_from_expr(&expressions[0], &ExprFactScope::new()),
        TypeFact::Union(vec![TypeFact::I64, TypeFact::NULL])
    );
}

#[test]
fn option_result_predicates_narrow_branch_facts() {
    let expressions = function_exprs(
        r#"
            fn main() {
                if option::is_some(maybe_player) { maybe_player } else { maybe_player };
                if !result::is_err(grant_result) { grant_result } else { grant_result };
                if maybe_player.is_none() { maybe_player } else { maybe_player };
                if grant_result.is_ok() { grant_result } else { grant_result };
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["maybe_player"], TypeFact::option(TypeFact::host("Player")))
        .with_path(
            ["grant_result"],
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
        );
    let maybe_player = vec!["maybe_player".to_owned()];
    let grant_result = vec!["grant_result".to_owned()];

    let ExprKind::If(option_if) = &expressions[0].kind else {
        panic!("expected option if expression");
    };
    let then_scope = scope.narrowed_by_condition(&option_if.condition, true);
    let else_scope = scope.narrowed_by_condition(&option_if.condition, false);
    assert_eq!(
        then_scope.path_fact(&maybe_player),
        Some(&TypeFact::option_some(TypeFact::host("Player")))
    );
    assert_eq!(
        else_scope.path_fact(&maybe_player),
        Some(&TypeFact::option_none())
    );

    let ExprKind::If(result_if) = &expressions[1].kind else {
        panic!("expected result if expression");
    };
    let then_scope = scope.narrowed_by_condition(&result_if.condition, true);
    let else_scope = scope.narrowed_by_condition(&result_if.condition, false);
    assert_eq!(
        then_scope.path_fact(&grant_result),
        Some(&TypeFact::result_ok(TypeFact::I64))
    );
    assert_eq!(
        else_scope.path_fact(&grant_result),
        Some(&TypeFact::result_err(TypeFact::STRING))
    );

    let ExprKind::If(option_method_if) = &expressions[2].kind else {
        panic!("expected option method if expression");
    };
    let then_scope = scope.narrowed_by_condition(&option_method_if.condition, true);
    let else_scope = scope.narrowed_by_condition(&option_method_if.condition, false);
    assert_eq!(
        then_scope.path_fact(&maybe_player),
        Some(&TypeFact::option_none())
    );
    assert_eq!(
        else_scope.path_fact(&maybe_player),
        Some(&TypeFact::option_some(TypeFact::host("Player")))
    );

    let ExprKind::If(result_method_if) = &expressions[3].kind else {
        panic!("expected result method if expression");
    };
    let then_scope = scope.narrowed_by_condition(&result_method_if.condition, true);
    let else_scope = scope.narrowed_by_condition(&result_method_if.condition, false);
    assert_eq!(
        then_scope.path_fact(&grant_result),
        Some(&TypeFact::result_ok(TypeFact::I64))
    );
    assert_eq!(
        else_scope.path_fact(&grant_result),
        Some(&TypeFact::result_err(TypeFact::STRING))
    );
}

#[test]
fn infers_stdlib_method_facts_with_lambda_parameters() {
    let expressions = function_exprs(
        r#"
            fn main() {
                rewards.map(|reward| reward);
                rewards.find(|reward| reward);
                scores.sum(|score| score);
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["rewards"], TypeFact::array(TypeFact::record("Reward")))
        .with_path(["scores"], TypeFact::array(TypeFact::I64));

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::array(TypeFact::record("Reward"))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::option(TypeFact::record("Reward"))
    );
    assert_eq!(type_fact_from_expr(&expressions[2], &scope), TypeFact::I64);
}

#[test]
fn infers_iterator_pipeline_facts_without_script_generics() {
    let expressions = function_exprs(
        r#"
            fn main() {
                scores.iter();
                scores.iter().map(|score| score + 1);
                scores.iter().map(|score| score + 1).collect_array();
                scores.iter().filter(|score| score > 2).find(|score| score > 4);
                scores.iter().take(2).skip(1).count();
                rewards.keys().collect_array();
                rewards.values().collect_array();
                rewards.entries().collect_array();
                (1..=4).iter().collect_array();
                "ab".chars().collect_array();
                "ab".bytes().collect_array();
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["scores"], TypeFact::array(TypeFact::I64))
        .with_path(["rewards"], TypeFact::map(TypeFact::STRING, TypeFact::I64));

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::iterator(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::iterator(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[2], &scope),
        TypeFact::array(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[3], &scope),
        TypeFact::option(TypeFact::I64)
    );
    assert_eq!(type_fact_from_expr(&expressions[4], &scope), TypeFact::I64);
    assert_eq!(
        type_fact_from_expr(&expressions[5], &scope),
        TypeFact::array(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[6], &scope),
        TypeFact::array(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[7], &scope),
        TypeFact::array(TypeFact::record("MapEntry"))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[8], &scope),
        TypeFact::array(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[9], &scope),
        TypeFact::array(TypeFact::CHAR)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[10], &scope),
        TypeFact::array(TypeFact::U8)
    );
}

#[test]
fn infers_value_fact_for_single_arg_map_callbacks() {
    let expressions = function_exprs(
        r#"
            fn main() {
                rewards.map_values(|amount| amount);
                rewards.map_values(|key, amount| key);
                rewards.filter(|amount| amount > 4);
                rewards.count(|| true);
            }
            "#,
    );
    let scope =
        ExprFactScope::new().with_path(["rewards"], TypeFact::map(TypeFact::STRING, TypeFact::I64));

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::map(TypeFact::STRING, TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::map(TypeFact::STRING, TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[2], &scope),
        TypeFact::map(TypeFact::STRING, TypeFact::I64)
    );
    assert_eq!(type_fact_from_expr(&expressions[3], &scope), TypeFact::I64);
}

#[test]
fn infers_option_result_map_method_facts() {
    let expressions = function_exprs(
        r#"
            fn main() {
                maybe.map(|value| value);
                some.map(|value| value);
                none.map(|value| value);
                grant.map(|value| value);
                failed.map(|value| value);
                grant.map_err(|error| error);
                failed.map_err(|error| error);
                ok.map_err(|error| error);
                maybe.and_then(|value| some);
                some.and_then(|value| none);
                grant.and_then(|value| ok);
                failed.and_then(|value| ok);
                ok.and_then(|value| failed);
                maybe.or_else(| | some);
                none.or_else(| | some);
                grant.or_else(|error| ok);
                failed.or_else(|error| ok);
                ok.or_else(|error| failed);
                maybe.filter(|value| value > 0);
                some.filter(|value| value.len() > 0);
                none.filter(|value| true);
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["maybe"], TypeFact::option(TypeFact::I64))
        .with_path(["some"], TypeFact::option_some(TypeFact::STRING))
        .with_path(["none"], TypeFact::option_none())
        .with_path(["grant"], TypeFact::result(TypeFact::STRING, TypeFact::I64))
        .with_path(["failed"], TypeFact::result_err(TypeFact::record("Error")))
        .with_path(["ok"], TypeFact::result_ok(TypeFact::STRING));

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::option(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::option_some(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[2], &scope),
        TypeFact::option_none()
    );
    assert_eq!(
        type_fact_from_expr(&expressions[3], &scope),
        TypeFact::result(TypeFact::STRING, TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[4], &scope),
        TypeFact::result_err(TypeFact::record("Error"))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[5], &scope),
        TypeFact::result(TypeFact::STRING, TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[6], &scope),
        TypeFact::result_err(TypeFact::record("Error"))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[7], &scope),
        TypeFact::result_ok(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[8], &scope),
        TypeFact::option(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[9], &scope),
        TypeFact::option_none()
    );
    assert_eq!(
        type_fact_from_expr(&expressions[10], &scope),
        TypeFact::result(TypeFact::STRING, TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[11], &scope),
        TypeFact::result_err(TypeFact::record("Error"))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[12], &scope),
        TypeFact::result_err(TypeFact::record("Error"))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[13], &scope),
        TypeFact::option(TypeFact::union([TypeFact::I64, TypeFact::STRING]))
    );
    assert_eq!(
        type_fact_from_expr(&expressions[14], &scope),
        TypeFact::option_some(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[15], &scope),
        TypeFact::result_ok(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[16], &scope),
        TypeFact::result_ok(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[17], &scope),
        TypeFact::result_ok(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[18], &scope),
        TypeFact::option(TypeFact::I64)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[19], &scope),
        TypeFact::option(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[20], &scope),
        TypeFact::option_none()
    );
}

#[test]
fn infers_stdlib_function_facts() {
    let expressions = function_exprs(
        r#"
            fn main() {
                option::unwrap_or(maybe, 10);
                set::from_array(names);
                math::pow(2, 3);
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["maybe"], TypeFact::option(TypeFact::I64))
        .with_path(["names"], TypeFact::array(TypeFact::STRING));

    assert_eq!(type_fact_from_expr(&expressions[0], &scope), TypeFact::I64);
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::set(TypeFact::STRING)
    );
    assert_eq!(
        type_fact_from_expr(&expressions[2], &scope),
        TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
    );
}

#[test]
fn infers_range_expression_facts() {
    let expressions = function_exprs(
        r#"
            fn main() {
                1..4;
                1..=4;
                (1..=4).len();
                (1..4).is_empty();
            }
            "#,
    );
    let scope = ExprFactScope::new();

    assert_eq!(
        type_fact_from_expr(&expressions[0], &scope),
        TypeFact::Range
    );
    assert_eq!(
        type_fact_from_expr(&expressions[1], &scope),
        TypeFact::Range
    );
    assert_eq!(type_fact_from_expr(&expressions[2], &scope), TypeFact::I64);
    assert_eq!(type_fact_from_expr(&expressions[3], &scope), TypeFact::BOOL);
}

#[test]
fn match_patterns_bind_variant_field_facts() {
    let expressions = function_exprs(
        r#"
            fn main(quest) {
                match quest {
                    QuestState::Active { quest_id } => quest_id.len(),
                    QuestState::Done => 0,
                };
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
    let facts = quest_registry_facts();

    assert_eq!(
        type_fact_from_expr_with_registry(&expressions[0], &scope, &facts),
        TypeFact::I64
    );
}

#[test]
fn match_patterns_narrow_scrutinee_variant_facts() {
    let expressions = function_exprs(
        r#"
            fn main(quest) {
                match quest {
                    QuestState::Active { quest_id } => quest.quest_id,
                    QuestState::Done => "",
                };
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
    let facts = quest_registry_facts();

    assert_eq!(
        type_fact_from_expr_with_registry(&expressions[0], &scope, &facts),
        TypeFact::STRING
    );
}

#[test]
fn option_match_patterns_bind_dynamic_payload_facts() {
    let expressions = function_exprs(
        r#"
            fn main(maybe_player) {
                match maybe_player {
                    Option::Some(player) => player.level,
                    Option::None => 0,
                };
            }
            "#,
    );
    let scope = ExprFactScope::new()
        .with_path(["maybe_player"], TypeFact::option(TypeFact::host("Player")));
    let facts = player_registry_facts();

    assert_eq!(
        type_fact_from_expr_with_registry(&expressions[0], &scope, &facts),
        TypeFact::I64
    );
}

#[test]
fn result_match_patterns_bind_dynamic_payload_facts() {
    let expressions = function_exprs(
        r#"
            fn main(grant_result) {
                match grant_result {
                    Result::Ok(player) => player.level,
                    Result::Err(reason) => reason.len(),
                };
            }
            "#,
    );
    let scope = ExprFactScope::new().with_path(
        ["grant_result"],
        TypeFact::result(TypeFact::host("Player"), TypeFact::STRING),
    );
    let facts = player_registry_facts();

    assert_eq!(
        type_fact_from_expr_with_registry(&expressions[0], &scope, &facts),
        TypeFact::I64
    );
}

fn quest_registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "QuestState"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "quest_id").type_hint("String")),
            )
            .variant(VariantDesc::new(VariantId::new(2), "Done")),
    );
    RegistryFacts::from_registry(&registry)
}

fn player_registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "Player"))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("i64")),
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
