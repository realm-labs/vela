use vela_common::Diagnostic;
use vela_syntax::{ElseBranch, Expr, ExprKind, MatchExpr, Pattern, Stmt, StmtKind};

use super::candidates::ranked_names;
use crate::{ExprFactScope, RegistryFacts, TypeFact, type_fact_from_expr};

pub fn match_pattern_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_match_pattern_diagnostics(expr, scope, facts, &mut diagnostics);
    diagnostics
}

fn collect_match_pattern_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_match_pattern_diagnostics(expr, scope, facts, diagnostics);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_match_pattern_diagnostics(left, scope, facts, diagnostics);
            collect_match_pattern_diagnostics(right, scope, facts, diagnostics);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_match_pattern_diagnostics(target, scope, facts, diagnostics);
            collect_match_pattern_diagnostics(value, scope, facts, diagnostics);
        }
        ExprKind::Field { base, .. } => {
            collect_match_pattern_diagnostics(base, scope, facts, diagnostics);
        }
        ExprKind::Call { callee, args } => {
            collect_match_pattern_diagnostics(callee, scope, facts, diagnostics);
            for arg in args {
                collect_match_pattern_diagnostics(&arg.value, scope, facts, diagnostics);
            }
        }
        ExprKind::Index { base, index } => {
            collect_match_pattern_diagnostics(base, scope, facts, diagnostics);
            collect_match_pattern_diagnostics(index, scope, facts, diagnostics);
        }
        ExprKind::Array(values) => {
            for value in values {
                collect_match_pattern_diagnostics(value, scope, facts, diagnostics);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_match_pattern_diagnostics(&entry.key, scope, facts, diagnostics);
                collect_match_pattern_diagnostics(&entry.value, scope, facts, diagnostics);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_match_pattern_diagnostics(value, scope, facts, diagnostics);
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_match_pattern_diagnostics(body, scope, facts, diagnostics);
        }
        ExprKind::If(if_expr) => {
            collect_match_pattern_diagnostics(&if_expr.condition, scope, facts, diagnostics);
            let then_scope = scope.narrowed_by_condition(&if_expr.condition, true);
            let else_scope = scope.narrowed_by_condition(&if_expr.condition, false);
            for statement in &if_expr.then_branch.statements {
                collect_statement_match_pattern_diagnostics(
                    statement,
                    &then_scope,
                    facts,
                    diagnostics,
                );
            }
            if let Some(else_branch) = &if_expr.else_branch {
                match else_branch {
                    ElseBranch::If(if_expr) => {
                        collect_match_pattern_diagnostics(
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
                            collect_statement_match_pattern_diagnostics(
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
            diagnose_match_patterns(expr, match_expr, scope, facts, diagnostics);
            collect_match_pattern_diagnostics(&match_expr.scrutinee, scope, facts, diagnostics);
            for arm in &match_expr.arms {
                let arm_scope =
                    scope.narrowed_by_match_pattern(&match_expr.scrutinee, &arm.pattern, facts);
                if let Some(guard) = &arm.guard {
                    collect_match_pattern_diagnostics(guard, &arm_scope, facts, diagnostics);
                }
                collect_match_pattern_diagnostics(&arm.body, &arm_scope, facts, diagnostics);
            }
        }
        ExprKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_match_pattern_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn collect_statement_match_pattern_diagnostics(
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
            collect_match_pattern_diagnostics(value, scope, facts, diagnostics);
        }
        StmtKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_match_pattern_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            collect_match_pattern_diagnostics(iterable, scope, facts, diagnostics);
            for statement in &body.statements {
                collect_statement_match_pattern_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        StmtKind::Return(None)
        | StmtKind::Let { value: None, .. }
        | StmtKind::Break
        | StmtKind::Continue => {}
    }
}

fn diagnose_match_patterns(
    expr: &Expr,
    match_expr: &MatchExpr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scrutinee_fact = type_fact_from_expr(&match_expr.scrutinee, scope);
    let Some(enum_shape) = enum_shape(&scrutinee_fact, facts) else {
        return;
    };
    for arm in &match_expr.arms {
        diagnose_pattern(expr, &arm.pattern, &enum_shape, diagnostics);
    }
}

fn diagnose_pattern(
    expr: &Expr,
    pattern: &Pattern,
    enum_shape: &EnumShape,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((owner, variant)) = pattern_variant_path(pattern) else {
        return;
    };
    if owner
        .as_ref()
        .is_some_and(|owner| owner != &enum_shape.name)
    {
        return;
    }
    if enum_shape.variants.iter().any(|known| known == variant) {
        return;
    }

    let candidates = ranked_names(variant, enum_shape.variants.iter().cloned());
    let mut diagnostic = Diagnostic::error(format!(
        "unknown variant `{variant}` for `{}`",
        enum_shape.name
    ))
    .with_code("analysis::unknown_variant")
    .with_span(expr.span)
    .with_label(expr.span, "unknown match pattern variant");
    if !candidates.is_empty() {
        diagnostic = diagnostic.with_label(
            expr.span,
            format!("available variants: {}", candidates.join(", ")),
        );
    }
    diagnostics.push(diagnostic);
}

fn pattern_variant_path(pattern: &Pattern) -> Option<(Option<String>, &str)> {
    let path = match pattern {
        Pattern::Path(path)
        | Pattern::TupleVariant { path, .. }
        | Pattern::RecordVariant { path, .. } => path,
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::Binding(_) => return None,
    };
    let (variant, owner) = path.split_last()?;
    Some((
        (!owner.is_empty()).then(|| owner.join(".")),
        variant.as_str(),
    ))
}

fn enum_shape(scrutinee_fact: &TypeFact, facts: &RegistryFacts) -> Option<EnumShape> {
    match scrutinee_fact {
        TypeFact::Enum { name, .. } => {
            let variants = facts.variant_names(name);
            if variants.is_empty() {
                None
            } else {
                Some(EnumShape {
                    name: name.clone(),
                    variants,
                })
            }
        }
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

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, TypeId, VariantId};
    use vela_reflect::{TypeDesc, TypeKey, TypeRegistry, VariantDesc};
    use vela_syntax::{ItemKind, StmtKind, parse_source};

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
                .kind(vela_reflect::TypeKind::ScriptEnum)
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
