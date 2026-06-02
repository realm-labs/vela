use std::collections::BTreeSet;

use vela_common::Diagnostic;
use vela_syntax::ast::{ElseBranch, Expr, ExprKind, MatchExpr, Pattern, Stmt, StmtKind};

use crate::expression::{ExprFactScope, type_fact_from_expr};
use crate::registry::RegistryFacts;
use crate::type_fact::TypeFact;

pub fn match_exhaustiveness_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_match_exhaustiveness_diagnostics(expr, scope, facts, &mut diagnostics);
    diagnostics
}

fn collect_match_exhaustiveness_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_match_exhaustiveness_diagnostics(expr, scope, facts, diagnostics);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_match_exhaustiveness_diagnostics(left, scope, facts, diagnostics);
            collect_match_exhaustiveness_diagnostics(right, scope, facts, diagnostics);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_match_exhaustiveness_diagnostics(target, scope, facts, diagnostics);
            collect_match_exhaustiveness_diagnostics(value, scope, facts, diagnostics);
        }
        ExprKind::Field { base, .. } => {
            collect_match_exhaustiveness_diagnostics(base, scope, facts, diagnostics);
        }
        ExprKind::Call { callee, args } => {
            collect_match_exhaustiveness_diagnostics(callee, scope, facts, diagnostics);
            for arg in args {
                collect_match_exhaustiveness_diagnostics(&arg.value, scope, facts, diagnostics);
            }
        }
        ExprKind::Index { base, index } => {
            collect_match_exhaustiveness_diagnostics(base, scope, facts, diagnostics);
            collect_match_exhaustiveness_diagnostics(index, scope, facts, diagnostics);
        }
        ExprKind::Array(values) => {
            for value in values {
                collect_match_exhaustiveness_diagnostics(value, scope, facts, diagnostics);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_match_exhaustiveness_diagnostics(&entry.key, scope, facts, diagnostics);
                collect_match_exhaustiveness_diagnostics(&entry.value, scope, facts, diagnostics);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_match_exhaustiveness_diagnostics(value, scope, facts, diagnostics);
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_match_exhaustiveness_diagnostics(body, scope, facts, diagnostics);
        }
        ExprKind::If(if_expr) => {
            collect_match_exhaustiveness_diagnostics(&if_expr.condition, scope, facts, diagnostics);
            let then_scope = scope.narrowed_by_condition(&if_expr.condition, true);
            let else_scope = scope.narrowed_by_condition(&if_expr.condition, false);
            for statement in &if_expr.then_branch.statements {
                collect_statement_match_diagnostics(statement, &then_scope, facts, diagnostics);
            }
            if let Some(else_branch) = &if_expr.else_branch {
                match else_branch {
                    ElseBranch::If(if_expr) => {
                        collect_match_exhaustiveness_diagnostics(
                            &Expr {
                                kind: ExprKind::If(if_expr.clone()),
                                span: if_expr.condition.span,
                            },
                            &else_scope,
                            facts,
                            diagnostics,
                        );
                    }
                    ElseBranch::Block(block) => {
                        for statement in &block.statements {
                            collect_statement_match_diagnostics(
                                statement,
                                &else_scope,
                                facts,
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }
        ExprKind::Match(match_expr) => {
            diagnose_match_exhaustiveness(expr, match_expr, scope, facts, diagnostics);
            collect_match_exhaustiveness_diagnostics(
                &match_expr.scrutinee,
                scope,
                facts,
                diagnostics,
            );
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_match_exhaustiveness_diagnostics(guard, scope, facts, diagnostics);
                }
                collect_match_exhaustiveness_diagnostics(&arm.body, scope, facts, diagnostics);
            }
        }
        ExprKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_match_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn collect_statement_match_diagnostics(
    statement: &Stmt,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &statement.kind {
        StmtKind::Let {
            value: Some(value), ..
        }
        | StmtKind::Return(Some(value))
        | StmtKind::Expr(value) => {
            collect_match_exhaustiveness_diagnostics(value, scope, facts, diagnostics);
        }
        StmtKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_match_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            collect_match_exhaustiveness_diagnostics(iterable, scope, facts, diagnostics);
            for statement in &body.statements {
                collect_statement_match_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        StmtKind::Return(None)
        | StmtKind::Let { value: None, .. }
        | StmtKind::Break
        | StmtKind::Continue => {}
    }
}

fn diagnose_match_exhaustiveness(
    expr: &Expr,
    match_expr: &MatchExpr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(enum_shape) = enum_shape(&type_fact_from_expr(&match_expr.scrutinee, scope), facts)
    else {
        return;
    };
    if enum_shape.variants.is_empty() || match_has_catch_all(match_expr) {
        return;
    }

    let covered = match_expr
        .arms
        .iter()
        .filter(|arm| arm.guard.is_none())
        .filter_map(|arm| pattern_variant_name(&arm.pattern))
        .collect::<BTreeSet<_>>();
    let missing = enum_shape
        .variants
        .into_iter()
        .filter(|variant| !covered.contains(variant))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return;
    }

    let mut diagnostic = Diagnostic::warning(format!(
        "match on `{}` does not cover all known variants",
        enum_shape.name
    ))
    .with_code("analysis::non_exhaustive_match")
    .with_span(expr.span)
    .with_label(
        expr.span,
        format!("missing variants: {}", missing.join(", ")),
    );
    if !match_expr.arms.iter().any(|arm| arm.guard.is_none()) {
        diagnostic = diagnostic.with_label(
            expr.span,
            "guarded arms do not make a match exhaustive for diagnostics",
        );
    }
    diagnostics.push(diagnostic);
}

fn enum_shape(scrutinee_fact: &TypeFact, facts: &RegistryFacts) -> Option<EnumShape> {
    match scrutinee_fact {
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(EnumShape {
            name: name.clone(),
            variants: facts.variant_names(name),
        }),
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some(EnumShape {
                name: "Option".to_owned(),
                variants: vec!["Some".to_owned(), "None".to_owned()],
            })
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some(EnumShape {
                name: "Result".to_owned(),
                variants: vec!["Ok".to_owned(), "Err".to_owned()],
            })
        }
        _ => None,
    }
}

struct EnumShape {
    name: String,
    variants: Vec<String>,
}

fn match_has_catch_all(match_expr: &MatchExpr) -> bool {
    match_expr.arms.iter().any(|arm| {
        arm.guard.is_none() && matches!(arm.pattern, Pattern::Wildcard | Pattern::Binding(_))
    })
}

fn pattern_variant_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Path(path)
        | Pattern::TupleVariant { path, .. }
        | Pattern::RecordVariant { path, .. } => path.last().cloned(),
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Binding(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, TypeId, VariantId};
    use vela_reflect::registry::{TypeDesc, TypeKey, TypeRegistry, VariantDesc};
    use vela_syntax::ast::{ItemKind, StmtKind};
    use vela_syntax::parser::parse_source;

    use super::*;

    #[test]
    fn reports_non_exhaustive_matches_for_known_enum_facts() {
        let exprs = function_exprs(
            r#"
            fn main(quest) {
                match quest {
                    QuestState.Active => 1,
                };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
        let facts = enum_registry_facts();

        let diagnostics = match_exhaustiveness_diagnostics(&exprs[0], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::non_exhaustive_match")
        );
        assert!(diagnostics[0].message.contains("QuestState"));
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message.contains("Finished"))
        );
    }

    #[test]
    fn treats_wildcard_match_arms_as_exhaustive() {
        let exprs = function_exprs(
            r#"
            fn main(quest) {
                match quest {
                    QuestState.Active => 1,
                    _ => 0,
                };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
        let facts = enum_registry_facts();

        assert!(match_exhaustiveness_diagnostics(&exprs[0], &scope, &facts).is_empty());
    }

    #[test]
    fn guarded_match_arms_do_not_count_as_exhaustive() {
        let exprs = function_exprs(
            r#"
            fn main(quest) {
                match quest {
                    QuestState.Active if ok => 1,
                    QuestState.Finished => 0,
                };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
        let facts = enum_registry_facts();

        let diagnostics = match_exhaustiveness_diagnostics(&exprs[0], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message.contains("Active"))
        );
    }

    #[test]
    fn reports_non_exhaustive_matches_for_option_and_result_facts() {
        let exprs = function_exprs(
            r#"
            fn main(maybe_reward, outcome) {
                match maybe_reward {
                    Option.Some(value) => value,
                };
                match outcome {
                    Result.Err(reason) => reason,
                };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["maybe_reward"], TypeFact::option(TypeFact::String))
            .with_path(
                ["outcome"],
                TypeFact::result(TypeFact::Int, TypeFact::String),
            );
        let facts = RegistryFacts::default();

        let option_diagnostics = match_exhaustiveness_diagnostics(&exprs[0], &scope, &facts);
        let result_diagnostics = match_exhaustiveness_diagnostics(&exprs[1], &scope, &facts);

        assert_eq!(option_diagnostics.len(), 1);
        assert!(option_diagnostics[0].message.contains("Option"));
        assert!(
            option_diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message.contains("None"))
        );
        assert_eq!(result_diagnostics.len(), 1);
        assert!(result_diagnostics[0].message.contains("Result"));
        assert!(
            result_diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message.contains("Ok"))
        );
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
}
