use vela_common::Diagnostic;
use vela_syntax::ast::{Expr, ExprKind};

use crate::completion::{CompletionKind, member_completions};
use crate::expression::{ExprFactScope, type_fact_from_expr};
use crate::registry::RegistryFacts;
use crate::stdlib::stdlib_method_fact;
use crate::type_fact::TypeFact;

use super::candidates::ranked_names;

pub fn member_access_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_member_access_diagnostics(expr, scope, facts, &mut diagnostics);
    diagnostics
}

fn collect_member_access_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_member_access_diagnostics(expr, scope, facts, diagnostics);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_member_access_diagnostics(left, scope, facts, diagnostics);
            collect_member_access_diagnostics(right, scope, facts, diagnostics);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_member_access_diagnostics(target, scope, facts, diagnostics);
            collect_member_access_diagnostics(value, scope, facts, diagnostics);
        }
        ExprKind::Field { base, name } => {
            collect_member_access_diagnostics(base, scope, facts, diagnostics);
            diagnose_field_access(expr, base, name, scope, facts, diagnostics);
        }
        ExprKind::Call { callee, args } => {
            let handled_member_call = diagnose_call(expr, callee, scope, facts, diagnostics);
            if handled_member_call {
                if let ExprKind::Field { base, .. } = &callee.kind {
                    collect_member_access_diagnostics(base, scope, facts, diagnostics);
                }
            } else {
                collect_member_access_diagnostics(callee, scope, facts, diagnostics);
            }
            for arg in args {
                collect_member_access_diagnostics(&arg.value, scope, facts, diagnostics);
            }
        }
        ExprKind::Index { base, index } => {
            collect_member_access_diagnostics(base, scope, facts, diagnostics);
            collect_member_access_diagnostics(index, scope, facts, diagnostics);
        }
        ExprKind::Array(values) => {
            for value in values {
                collect_member_access_diagnostics(value, scope, facts, diagnostics);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_member_access_diagnostics(&entry.key, scope, facts, diagnostics);
                collect_member_access_diagnostics(&entry.value, scope, facts, diagnostics);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_member_access_diagnostics(value, scope, facts, diagnostics);
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_member_access_diagnostics(body, scope, facts, diagnostics);
        }
        ExprKind::If(if_expr) => {
            collect_if_expr_diagnostics(if_expr, scope, facts, diagnostics);
        }
        ExprKind::Match(match_expr) => {
            collect_member_access_diagnostics(&match_expr.scrutinee, scope, facts, diagnostics);
            for arm in &match_expr.arms {
                let arm_scope =
                    scope.narrowed_by_match_pattern(&match_expr.scrutinee, &arm.pattern, facts);
                if let Some(guard) = &arm.guard {
                    collect_member_access_diagnostics(guard, &arm_scope, facts, diagnostics);
                }
                collect_member_access_diagnostics(&arm.body, &arm_scope, facts, diagnostics);
            }
        }
        ExprKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        ExprKind::Path(path) => {
            diagnose_path_field_access(expr, path, scope, facts, diagnostics);
        }
        ExprKind::Literal(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn collect_if_expr_diagnostics(
    if_expr: &vela_syntax::ast::IfExpr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_member_access_diagnostics(&if_expr.condition, scope, facts, diagnostics);
    let then_scope = scope.narrowed_by_condition(&if_expr.condition, true);
    let else_scope = scope.narrowed_by_condition(&if_expr.condition, false);
    for statement in &if_expr.then_branch.statements {
        collect_statement_diagnostics(statement, &then_scope, facts, diagnostics);
    }
    if let Some(else_branch) = &if_expr.else_branch {
        match else_branch {
            vela_syntax::ast::ElseBranch::If(if_expr) => {
                collect_if_expr_diagnostics(if_expr, &else_scope, facts, diagnostics);
            }
            vela_syntax::ast::ElseBranch::Block(block) => {
                for statement in &block.statements {
                    collect_statement_diagnostics(statement, &else_scope, facts, diagnostics);
                }
            }
        }
    }
}

fn collect_statement_diagnostics(
    statement: &vela_syntax::ast::Stmt,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &statement.kind {
        vela_syntax::ast::StmtKind::Let {
            value: Some(value), ..
        }
        | vela_syntax::ast::StmtKind::Return(Some(value))
        | vela_syntax::ast::StmtKind::Expr(value) => {
            collect_member_access_diagnostics(value, scope, facts, diagnostics);
        }
        vela_syntax::ast::StmtKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        vela_syntax::ast::StmtKind::For { iterable, body, .. } => {
            collect_member_access_diagnostics(iterable, scope, facts, diagnostics);
            for statement in &body.statements {
                collect_statement_diagnostics(statement, scope, facts, diagnostics);
            }
        }
        vela_syntax::ast::StmtKind::Return(None)
        | vela_syntax::ast::StmtKind::Let { value: None, .. }
        | vela_syntax::ast::StmtKind::Break
        | vela_syntax::ast::StmtKind::Continue => {}
    }
}

fn diagnose_call(
    expr: &Expr,
    callee: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some((receiver, name)) = member_receiver_and_name(callee, scope) else {
        return false;
    };

    if !is_precise_receiver(&receiver) {
        return false;
    }
    if method_exists(facts, &receiver, &name) {
        return true;
    }

    diagnostics.push(unknown_member_diagnostic(
        "analysis::unknown_method",
        format!("unknown method `{name}` for `{}`", receiver.display_name()),
        expr,
        ranked_member_candidates(facts, &receiver, &name, CompletionKind::Method),
    ));
    true
}

fn diagnose_field_access(
    expr: &Expr,
    base: &Expr,
    field: &str,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver = type_fact_from_expr(base, scope);
    if !is_precise_receiver(&receiver) || field_exists(facts, &receiver, field) {
        return;
    }

    diagnostics.push(unknown_member_diagnostic(
        "analysis::unknown_field",
        format!("unknown field `{field}` for `{}`", receiver.display_name()),
        expr,
        ranked_member_candidates(facts, &receiver, field, CompletionKind::Field),
    ));
}

fn diagnose_path_field_access(
    expr: &Expr,
    path: &[String],
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((receiver, field)) = path_receiver_and_name(expr, path, scope) else {
        return;
    };
    if !is_precise_receiver(&receiver) || field_exists(facts, &receiver, &field) {
        return;
    }

    diagnostics.push(unknown_member_diagnostic(
        "analysis::unknown_field",
        format!("unknown field `{field}` for `{}`", receiver.display_name()),
        expr,
        ranked_member_candidates(facts, &receiver, &field, CompletionKind::Field),
    ));
}

fn member_receiver_and_name(expr: &Expr, scope: &ExprFactScope) -> Option<(TypeFact, String)> {
    match &expr.kind {
        ExprKind::Field { base, name } => Some((type_fact_from_expr(base, scope), name.clone())),
        ExprKind::Path(path) => path_receiver_and_name(expr, path, scope),
        _ => None,
    }
}

fn path_receiver_and_name(
    expr: &Expr,
    path: &[String],
    scope: &ExprFactScope,
) -> Option<(TypeFact, String)> {
    let (name, receiver_path) = path.split_last()?;
    if receiver_path.is_empty() {
        return None;
    }
    let receiver = Expr {
        kind: ExprKind::Path(receiver_path.to_vec()),
        span: expr.span,
    };
    Some((type_fact_from_expr(&receiver, scope), name.clone()))
}

fn is_precise_receiver(receiver: &TypeFact) -> bool {
    matches!(
        receiver,
        TypeFact::Host { .. }
            | TypeFact::Record { .. }
            | TypeFact::Enum {
                variant: Some(_),
                ..
            }
            | TypeFact::Array { .. }
            | TypeFact::Map { .. }
            | TypeFact::Set { .. }
            | TypeFact::String
            | TypeFact::Trait { .. }
    )
}

fn field_exists(facts: &RegistryFacts, receiver: &TypeFact, field: &str) -> bool {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            facts.field_fact(name, field).is_some()
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => facts
            .field_fact(&format!("{name}.{variant}"), field)
            .is_some(),
        _ => false,
    }
}

fn method_exists(facts: &RegistryFacts, receiver: &TypeFact, method: &str) -> bool {
    if stdlib_method_fact(receiver, method, None).is_some() {
        return true;
    }

    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            facts.method_fact(name, method).is_some()
        }
        TypeFact::Trait { name } => facts.trait_method_fact(name, method).is_some(),
        _ => false,
    }
}

fn candidates(facts: &RegistryFacts, receiver: &TypeFact, kind: CompletionKind) -> Vec<String> {
    member_completions(facts, receiver)
        .into_iter()
        .filter(|completion| completion.kind == kind)
        .map(|completion| completion.label)
        .collect()
}

fn ranked_member_candidates(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    name: &str,
    kind: CompletionKind,
) -> Vec<String> {
    ranked_names(name, candidates(facts, receiver, kind))
}

fn unknown_member_diagnostic(
    code: &'static str,
    message: String,
    expr: &Expr,
    candidates: Vec<String>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(message)
        .with_code(code)
        .with_span(expr.span)
        .with_label(expr.span, "unknown member access");
    if let Some(candidate) = candidates.first() {
        diagnostic = diagnostic.with_label(expr.span, format!("did you mean `{candidate}`?"));
    }
    if candidates.len() > 1 {
        diagnostic = diagnostic.with_label(
            expr.span,
            format!("similar candidates: {}", candidates.join(", ")),
        );
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, HostMethodId, SourceId, TypeId};
    use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey, TypeRegistry};
    use vela_syntax::ast::{ItemKind, StmtKind};
    use vela_syntax::parser::parse_source;

    use super::*;

    #[test]
    fn reports_unknown_fields_for_known_registry_receiver() {
        let exprs = function_exprs(
            r#"
            fn main(player) {
                player.level;
                player.levle;
            }
            "#,
        );
        let scope = ExprFactScope::new().with_path(["player"], TypeFact::host("Player"));
        let facts = registry_facts();

        assert!(member_access_diagnostics(&exprs[0], &scope, &facts).is_empty());
        let diagnostics = member_access_diagnostics(&exprs[1], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::unknown_field")
        );
        assert!(diagnostics[0].message.contains("levle"));
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message == "did you mean `level`?")
        );
    }

    #[test]
    fn reports_unknown_methods_for_known_registry_receiver() {
        let exprs = function_exprs(
            r#"
            fn main(player) {
                player.grant_exp(10);
                player.grant_xp(10);
            }
            "#,
        );
        let scope = ExprFactScope::new().with_path(["player"], TypeFact::host("Player"));
        let facts = registry_facts();

        assert!(member_access_diagnostics(&exprs[0], &scope, &facts).is_empty());
        let diagnostics = member_access_diagnostics(&exprs[1], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::unknown_method")
        );
        assert!(diagnostics[0].message.contains("grant_xp"));
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message == "did you mean `grant_exp`?")
        );
    }

    #[test]
    fn allows_stdlib_methods_and_dynamic_receivers() {
        let exprs = function_exprs(
            r#"
            fn main(scores, value) {
                scores.map(|score| score);
                value.anything();
            }
            "#,
        );
        let scope = ExprFactScope::new().with_path(["scores"], TypeFact::array(TypeFact::Int));
        let facts = RegistryFacts::default();

        assert!(member_access_diagnostics(&exprs[0], &scope, &facts).is_empty());
        assert!(member_access_diagnostics(&exprs[1], &scope, &facts).is_empty());
    }

    #[test]
    fn null_checks_narrow_member_diagnostics_inside_branches() {
        let exprs = function_exprs(
            r#"
            fn main(player) {
                if player != null {
                    player.missing;
                }
            }
            "#,
        );
        let scope = ExprFactScope::new().with_path(
            ["player"],
            TypeFact::Union(vec![TypeFact::Null, TypeFact::host("Player")]),
        );
        let facts = registry_facts();

        let diagnostics = member_access_diagnostics(&exprs[0], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::unknown_field")
        );
        assert!(diagnostics[0].message.contains("missing"));
    }

    #[test]
    fn match_patterns_narrow_member_diagnostics_inside_arms() {
        let exprs = function_exprs(
            r#"
            fn main(quest) {
                match quest {
                    QuestState.Active { quest_id } => {
                        quest.quest_id;
                        quest.missing;
                        quest_id.len();
                    }
                    QuestState.Done => {}
                };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["quest"], TypeFact::enum_type("QuestState", None::<String>));
        let facts = quest_registry_facts();

        let diagnostics = member_access_diagnostics(&exprs[0], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::unknown_field")
        );
        assert!(diagnostics[0].message.contains("missing"));
        assert!(diagnostics[0].message.contains("QuestState.Active"));
    }

    #[test]
    fn option_match_patterns_narrow_payload_member_diagnostics() {
        let exprs = function_exprs(
            r#"
            fn main(maybe_player) {
                match maybe_player {
                    Option.Some(player) => player.missing,
                    Option.None => null,
                };
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["maybe_player"], TypeFact::option(TypeFact::host("Player")));
        let facts = registry_facts();

        let diagnostics = member_access_diagnostics(&exprs[0], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::unknown_field")
        );
        assert!(diagnostics[0].message.contains("Player"));
        assert!(diagnostics[0].message.contains("missing"));
    }

    fn registry_facts() -> RegistryFacts {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "level").type_hint("int"))
                .method(MethodDesc::new(HostMethodId::new(1), "grant_exp")),
        );
        RegistryFacts::from_registry(&registry)
    }

    fn quest_registry_facts() -> RegistryFacts {
        use vela_common::VariantId;
        use vela_reflect::registry::{TypeKind, VariantDesc};

        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestState"))
                .kind(TypeKind::ScriptEnum)
                .variant(
                    VariantDesc::new(VariantId::new(1), "Active")
                        .field(FieldDesc::new(FieldId::new(1), "quest_id").type_hint("string")),
                )
                .variant(VariantDesc::new(VariantId::new(2), "Done")),
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
