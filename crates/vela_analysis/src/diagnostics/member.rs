use vela_common::{Diagnostic, PrimitiveTag};
use vela_syntax::ast::{AssignOp, Expr, ExprKind, InterpolatedStringPart};

use crate::completion::{CompletionKind, member_completions};
use crate::expression::{ExprFactScope, type_fact_from_expr};
use crate::registry::{
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryIndexCapabilityFact,
    RegistryMethodAccessFact,
};
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
            diagnose_assignment_target(target, scope, facts, diagnostics);
            if let ExprKind::Index { base, index } = &target.kind {
                collect_member_access_diagnostics(base, scope, facts, diagnostics);
                collect_member_access_diagnostics(index, scope, facts, diagnostics);
                diagnose_index_access(
                    target,
                    base,
                    index,
                    scope,
                    facts,
                    host_index_assignment_kind(expr),
                    diagnostics,
                );
            } else {
                collect_member_access_diagnostics(target, scope, facts, diagnostics);
            }
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
            diagnose_index_access(
                expr,
                base,
                index,
                scope,
                facts,
                HostIndexDiagnosticAccessKind::Read,
                diagnostics,
            );
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
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let InterpolatedStringPart::Expr(expr) = part {
                    collect_member_access_diagnostics(expr, scope, facts, diagnostics);
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

    let candidates = ranked_member_candidates(facts, &receiver, &name, CompletionKind::Method);
    let access_labels = method_candidate_access_labels(facts, &receiver, &candidates);
    diagnostics.push(unknown_member_diagnostic(
        "analysis::unknown_method",
        format!("unknown method `{name}` for `{}`", receiver.display_name()),
        expr,
        candidates,
        access_labels,
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

    let candidates = ranked_member_candidates(facts, &receiver, field, CompletionKind::Field);
    let access_labels = field_candidate_access_labels(facts, &receiver, &candidates);
    diagnostics.push(unknown_member_diagnostic(
        "analysis::unknown_field",
        format!("unknown field `{field}` for `{}`", receiver.display_name()),
        expr,
        candidates,
        access_labels,
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

    let candidates = ranked_member_candidates(facts, &receiver, &field, CompletionKind::Field);
    let access_labels = field_candidate_access_labels(facts, &receiver, &candidates);
    diagnostics.push(unknown_member_diagnostic(
        "analysis::unknown_field",
        format!("unknown field `{field}` for `{}`", receiver.display_name()),
        expr,
        candidates,
        access_labels,
    ));
}

fn diagnose_assignment_target(
    target: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((receiver, field)) = member_receiver_and_name(target, scope) else {
        return;
    };
    if !is_precise_receiver(&receiver) {
        return;
    }
    let Some(access) = field_access(facts, &receiver, &field) else {
        return;
    };
    if access.writable {
        return;
    }

    diagnostics.push(
        Diagnostic::error(format!(
            "field `{}.{}` is read-only for script writes",
            access.owner, access.name
        ))
        .with_code("analysis::field_not_writable")
        .with_span(target.span)
        .with_label(target.span, "assignment targets a read-only field")
        .with_label(
            target.span,
            "write through an exposed method or a writable field instead",
        ),
    );
}

fn diagnose_index_access(
    expr: &Expr,
    base: &Expr,
    index: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    kind: HostIndexDiagnosticAccessKind,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver = type_fact_from_expr(base, scope);
    let TypeFact::Host { name } = receiver else {
        return;
    };
    let index_fact = type_fact_from_expr(index, scope);
    let Some(capability) = facts.index_capability_fact(&name) else {
        diagnostics.push(
            Diagnostic::error(format!(
                "type `{name}` does not support host index {}",
                kind.access_name()
            ))
            .with_code("analysis::host_index_not_supported")
            .with_span(expr.span)
            .with_label(
                expr.span,
                "host index access is not registered for this type",
            )
            .with_label(
                base.span,
                "register a host index capability or expose a field/method instead",
            ),
        );
        return;
    };
    if !kind.allowed_by(capability) {
        diagnostics.push(
            Diagnostic::error(format!(
                "type `{name}` does not allow host index {}",
                kind.access_name()
            ))
            .with_code(kind.denied_code())
            .with_span(expr.span)
            .with_label(expr.span, kind.capability_label())
            .with_label(base.span, kind.enable_label()),
        );
        return;
    }
    if !accepts_index_key(&index_fact, &capability.key) {
        diagnostics.push(host_index_key_mismatch_diagnostic(
            expr,
            index,
            capability,
            &index_fact,
        ));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostIndexDiagnosticAccessKind {
    Read,
    Write,
    Mutate,
}

impl HostIndexDiagnosticAccessKind {
    fn allowed_by(self, capability: &RegistryIndexCapabilityFact) -> bool {
        match self {
            Self::Read => capability.readable,
            Self::Write => capability.writable,
            Self::Mutate => capability.addable,
        }
    }

    const fn denied_code(self) -> &'static str {
        match self {
            Self::Read => "analysis::host_index_not_readable",
            Self::Write => "analysis::host_index_not_writable",
            Self::Mutate => "analysis::host_index_not_mutable",
        }
    }

    const fn access_name(self) -> &'static str {
        match self {
            Self::Read => "reads",
            Self::Write => "writes",
            Self::Mutate => "mutations",
        }
    }

    const fn capability_label(self) -> &'static str {
        match self {
            Self::Read => "host index capability is not readable",
            Self::Write => "host index capability is not writable",
            Self::Mutate => "host index capability is not addable",
        }
    }

    const fn enable_label(self) -> &'static str {
        match self {
            Self::Read => "enable readable host index access for this type",
            Self::Write => "enable writable host index access for this type",
            Self::Mutate => "enable addable host index access for this type",
        }
    }
}

fn host_index_assignment_kind(expr: &Expr) -> HostIndexDiagnosticAccessKind {
    let ExprKind::Assign { op, .. } = &expr.kind else {
        return HostIndexDiagnosticAccessKind::Read;
    };
    match op {
        AssignOp::Set => HostIndexDiagnosticAccessKind::Write,
        AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
            HostIndexDiagnosticAccessKind::Mutate
        }
    }
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
            | TypeFact::Primitive(PrimitiveTag::String)
            | TypeFact::Trait { .. }
    )
}

fn field_exists(facts: &RegistryFacts, receiver: &TypeFact, field: &str) -> bool {
    field_owner(receiver)
        .as_deref()
        .and_then(|owner| facts.field_fact(owner, field))
        .is_some()
}

fn field_access<'a>(
    facts: &'a RegistryFacts,
    receiver: &TypeFact,
    field: &str,
) -> Option<&'a RegistryFieldAccessFact> {
    match receiver {
        TypeFact::Host { name } => facts.field_access_fact(name, field),
        _ => None,
    }
}

fn field_candidate_access_labels(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    candidates: &[String],
) -> Vec<String> {
    let TypeFact::Host { name: owner } = receiver else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| {
            facts
                .field_access_fact(owner, candidate)
                .map(field_access_label)
        })
        .collect()
}

fn field_access_label(access: &RegistryFieldAccessFact) -> String {
    let read_hint = if access.readable {
        "readable"
    } else {
        "not script-readable"
    };
    let write_hint = if access.writable {
        "writable"
    } else {
        "read-only"
    };
    let mut label = format!(
        "candidate `{}.{}` is {read_hint} and {write_hint}",
        access.owner, access.name
    );
    if !access.required_permissions.is_empty() {
        label.push_str(&format!(
            "; requires permission {}",
            access.required_permissions.join(", ")
        ));
    }
    label
}

fn method_candidate_access_labels(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    candidates: &[String],
) -> Vec<String> {
    let TypeFact::Host { name: owner } = receiver else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| {
            let access = facts.method_access_fact(owner, candidate)?;
            let effects = facts.method_effect_fact(owner, candidate);
            Some(method_access_label(access, effects))
        })
        .collect()
}

fn method_access_label(
    access: &RegistryMethodAccessFact,
    effects: Option<&RegistryEffectFact>,
) -> String {
    let callable_hint = if access.reflect_callable {
        "reflect-callable"
    } else {
        "not reflect-callable"
    };
    let visibility_hint = if access.public { "public" } else { "private" };
    let effect_hint =
        effects.map_or_else(|| "unknown".to_owned(), RegistryEffectFact::display_name);
    let mut label = format!(
        "candidate `{}.{}` is {visibility_hint} and {callable_hint} with effects {effect_hint}",
        access.owner, access.name
    );
    if !access.required_permissions.is_empty() {
        label.push_str(&format!(
            "; requires permission {}",
            access.required_permissions.join(", ")
        ));
    }
    label
}

fn field_owner(receiver: &TypeFact) -> Option<String> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => Some(name.clone()),
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        _ => None,
    }
}

fn host_index_key_mismatch_diagnostic(
    expr: &Expr,
    index: &Expr,
    capability: &RegistryIndexCapabilityFact,
    actual: &TypeFact,
) -> Diagnostic {
    Diagnostic::error(format!(
        "host index key for `{}` must be `{}`",
        capability.owner,
        capability.key.display_name()
    ))
    .with_code("analysis::host_index_key_mismatch")
    .with_span(expr.span)
    .with_label(
        index.span,
        format!("index expression has type `{}`", actual.display_name()),
    )
}

fn accepts_index_key(index: &TypeFact, key: &TypeFact) -> bool {
    match (index, key) {
        (TypeFact::Any | TypeFact::Unknown, _) | (_, TypeFact::Any | TypeFact::Unknown) => true,
        (TypeFact::Union(facts), key) => facts.iter().any(|fact| accepts_index_key(fact, key)),
        (index, TypeFact::Union(facts)) => facts.iter().any(|fact| accepts_index_key(index, fact)),
        _ => key == index,
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
    extra_labels: Vec<String>,
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
    for label in extra_labels {
        diagnostic = diagnostic.with_label(expr.span, label);
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use vela_common::{HostMethodId, SourceId};
    use vela_def::{FieldId, TypeId};
    use vela_reflect::access::{MethodAccess, MethodEffectSet};
    use vela_reflect::registry::{
        FieldDesc, HostIndexCapability, MethodDesc, TypeDesc, TypeKey, TypeRegistry,
    };
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
                player.inventroy;
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
        assert!(diagnostics[0].labels.iter().any(|label| {
            label
                .message
                .contains("candidate `Player.level` is readable and writable")
        }));

        let diagnostics = member_access_diagnostics(&exprs[2], &scope, &facts);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::unknown_field")
        );
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message == "did you mean `inventory`?")
        );
        assert!(diagnostics[0].labels.iter().any(|label| {
            label
                .message
                .contains("candidate `Player.inventory` is readable and read-only")
        }));
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
        assert!(diagnostics[0].labels.iter().any(|label| {
            label.message.contains(
                "candidate `Player.grant_exp` is public and reflect-callable with effects writes_host",
            )
        }));
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| { label.message.contains("requires permission player.reward") })
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
        let scope = ExprFactScope::new().with_path(["scores"], TypeFact::array(TypeFact::I64));
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
            TypeFact::Union(vec![TypeFact::NULL, TypeFact::host("Player")]),
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
                    QuestState::Active { quest_id } => {
                        quest.quest_id;
                        quest.missing;
                        quest_id.len();
                    }
                    QuestState::Done => {}
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
        assert!(diagnostics[0].message.contains("QuestState::Active"));
    }

    #[test]
    fn option_match_patterns_narrow_payload_member_diagnostics() {
        let exprs = function_exprs(
            r#"
            fn main(maybe_player) {
                match maybe_player {
                    Option::Some(player) => player.missing,
                    Option::None => null,
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

    #[test]
    fn reports_read_only_host_field_assignment_hints() {
        let exprs = function_exprs(
            r#"
            fn main(player) {
                player.level = 2;
                player.inventory = 1;
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
            Some("analysis::field_not_writable")
        );
        assert!(diagnostics[0].message.contains("Player.inventory"));
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.message == "assignment targets a read-only field")
        );
        assert!(diagnostics[0].labels.iter().any(|label| {
            label
                .message
                .contains("write through an exposed method or a writable field")
        }));
    }

    #[test]
    fn reports_host_index_capability_diagnostics() {
        let exprs = function_exprs(
            r#"
            fn main(scores, write_only, unknown, read_only, frozen) {
                scores[0];
                scores["bad"];
                write_only[0];
                unknown[0];
                read_only[0] = 1;
                frozen[0] += 1;
            }
            "#,
        );
        let scope = ExprFactScope::new()
            .with_path(["scores"], TypeFact::host("Scores"))
            .with_path(["write_only"], TypeFact::host("WriteOnlyScores"))
            .with_path(["unknown"], TypeFact::host("UnindexedScores"))
            .with_path(["read_only"], TypeFact::host("ReadOnlyScores"))
            .with_path(["frozen"], TypeFact::host("FrozenScores"));
        let facts = host_index_registry_facts();

        assert!(member_access_diagnostics(&exprs[0], &scope, &facts).is_empty());

        let diagnostics = member_access_diagnostics(&exprs[1], &scope, &facts);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::host_index_key_mismatch")
        );
        assert!(diagnostics[0].message.contains("Scores"));
        assert!(diagnostics[0].message.contains("i64"));

        let diagnostics = member_access_diagnostics(&exprs[2], &scope, &facts);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::host_index_not_readable")
        );

        let diagnostics = member_access_diagnostics(&exprs[3], &scope, &facts);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::host_index_not_supported")
        );

        let diagnostics = member_access_diagnostics(&exprs[4], &scope, &facts);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::host_index_not_writable")
        );

        let diagnostics = member_access_diagnostics(&exprs[5], &scope, &facts);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("analysis::host_index_not_mutable")
        );
    }

    fn registry_facts() -> RegistryFacts {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(
                    FieldDesc::new(FieldId::new(1), "level")
                        .type_hint("i64")
                        .writable(true),
                )
                .field(FieldDesc::new(FieldId::new(2), "inventory").type_hint("map"))
                .method(
                    MethodDesc::new(HostMethodId::new(1), "grant_exp")
                        .effects(MethodEffectSet::host_write())
                        .access(MethodAccess::new().require_permission("player.reward")),
                ),
        );
        RegistryFacts::from_registry(&registry)
    }

    fn host_index_registry_facts() -> RegistryFacts {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(11), "Scores")).index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .key_type("i64")
                    .value_type("i64"),
            ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(12), "WriteOnlyScores")).index_capability(
                HostIndexCapability::new()
                    .writable(true)
                    .key_type("i64")
                    .value_type("i64"),
            ),
        );
        registry.register(TypeDesc::new(TypeKey::new(
            TypeId::new(13),
            "UnindexedScores",
        )));
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(14), "ReadOnlyScores")).index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .key_type("i64")
                    .value_type("i64"),
            ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(15), "FrozenScores")).index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .writable(true)
                    .key_type("i64")
                    .value_type("i64"),
            ),
        );
        RegistryFacts::from_registry(&registry)
    }

    fn quest_registry_facts() -> RegistryFacts {
        use vela_def::VariantId;
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
