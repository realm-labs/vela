use vela_common::Diagnostic;
use vela_syntax::{ElseBranch, Expr, ExprKind, Stmt, StmtKind};

use crate::{
    ExprFactScope, RegistryEffectFact, RegistryFacts, TypeFact, type_fact_from_expr_with_registry,
};

#[cfg(test)]
mod tests;

pub fn effect_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    allowed: &RegistryEffectFact,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_effect_diagnostics(expr, scope, facts, allowed, &mut diagnostics);
    diagnostics
}

fn collect_effect_diagnostics(
    expr: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    allowed: &RegistryEffectFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_effect_diagnostics(expr, scope, facts, allowed, diagnostics);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_effect_diagnostics(left, scope, facts, allowed, diagnostics);
            collect_effect_diagnostics(right, scope, facts, allowed, diagnostics);
        }
        ExprKind::Assign { target, value, .. } => {
            collect_effect_diagnostics(target, scope, facts, allowed, diagnostics);
            collect_effect_diagnostics(value, scope, facts, allowed, diagnostics);
        }
        ExprKind::Field { base, .. } => {
            collect_effect_diagnostics(base, scope, facts, allowed, diagnostics);
        }
        ExprKind::Call { callee, args } => {
            diagnose_call_effect(expr, callee, scope, facts, allowed, diagnostics);
            collect_effect_diagnostics(callee, scope, facts, allowed, diagnostics);
            for arg in args {
                collect_effect_diagnostics(&arg.value, scope, facts, allowed, diagnostics);
            }
        }
        ExprKind::Index { base, index } => {
            collect_effect_diagnostics(base, scope, facts, allowed, diagnostics);
            collect_effect_diagnostics(index, scope, facts, allowed, diagnostics);
        }
        ExprKind::Array(values) => {
            for value in values {
                collect_effect_diagnostics(value, scope, facts, allowed, diagnostics);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_effect_diagnostics(&entry.key, scope, facts, allowed, diagnostics);
                collect_effect_diagnostics(&entry.value, scope, facts, allowed, diagnostics);
            }
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_effect_diagnostics(value, scope, facts, allowed, diagnostics);
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_effect_diagnostics(body, scope, facts, allowed, diagnostics);
        }
        ExprKind::If(if_expr) => {
            collect_effect_diagnostics(&if_expr.condition, scope, facts, allowed, diagnostics);
            let then_scope = scope.narrowed_by_condition(&if_expr.condition, true);
            let else_scope = scope.narrowed_by_condition(&if_expr.condition, false);
            for statement in &if_expr.then_branch.statements {
                collect_statement_effect_diagnostics(
                    statement,
                    &then_scope,
                    facts,
                    allowed,
                    diagnostics,
                );
            }
            if let Some(else_branch) = &if_expr.else_branch {
                collect_else_effect_diagnostics(
                    else_branch,
                    &else_scope,
                    facts,
                    allowed,
                    diagnostics,
                );
            }
        }
        ExprKind::Match(match_expr) => {
            collect_effect_diagnostics(&match_expr.scrutinee, scope, facts, allowed, diagnostics);
            for arm in &match_expr.arms {
                let arm_scope =
                    scope.narrowed_by_match_pattern(&match_expr.scrutinee, &arm.pattern, facts);
                if let Some(guard) = &arm.guard {
                    collect_effect_diagnostics(guard, &arm_scope, facts, allowed, diagnostics);
                }
                collect_effect_diagnostics(&arm.body, &arm_scope, facts, allowed, diagnostics);
            }
        }
        ExprKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_effect_diagnostics(statement, scope, facts, allowed, diagnostics);
            }
        }
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn collect_else_effect_diagnostics(
    else_branch: &ElseBranch,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    allowed: &RegistryEffectFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match else_branch {
        ElseBranch::If(if_expr) => collect_effect_diagnostics(
            &Expr {
                kind: ExprKind::If(if_expr.clone()),
                span: if_expr.condition.span,
            },
            scope,
            facts,
            allowed,
            diagnostics,
        ),
        ElseBranch::Block(block) => {
            for statement in &block.statements {
                collect_statement_effect_diagnostics(statement, scope, facts, allowed, diagnostics);
            }
        }
    }
}

fn collect_statement_effect_diagnostics(
    statement: &Stmt,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    allowed: &RegistryEffectFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &statement.kind {
        StmtKind::Let {
            value: Some(value), ..
        }
        | StmtKind::Return(Some(value))
        | StmtKind::Expr(value) => {
            collect_effect_diagnostics(value, scope, facts, allowed, diagnostics);
        }
        StmtKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_effect_diagnostics(statement, scope, facts, allowed, diagnostics);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            collect_effect_diagnostics(iterable, scope, facts, allowed, diagnostics);
            for statement in &body.statements {
                collect_statement_effect_diagnostics(statement, scope, facts, allowed, diagnostics);
            }
        }
        StmtKind::Return(None)
        | StmtKind::Let { value: None, .. }
        | StmtKind::Break
        | StmtKind::Continue => {}
    }
}

fn diagnose_call_effect(
    expr: &Expr,
    callee: &Expr,
    scope: &ExprFactScope,
    facts: &RegistryFacts,
    allowed: &RegistryEffectFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((label, effect)) = call_effect(callee, scope, facts) else {
        return;
    };
    let denied = effect.denied_by(allowed);
    if denied.is_empty() {
        return;
    }

    diagnostics.push(
        Diagnostic::error(format!(
            "`{label}` uses denied effect{}: {}",
            if denied.len() == 1 { "" } else { "s" },
            denied.join(", ")
        ))
        .with_code("analysis::disallowed_effect")
        .with_span(expr.span)
        .with_label(
            expr.span,
            format!("allowed effects: {}", allowed.display_name()),
        )
        .with_label(
            expr.span,
            format!("call effects: {}", effect.display_name()),
        ),
    );
}

fn call_effect<'a>(
    callee: &Expr,
    scope: &ExprFactScope,
    facts: &'a RegistryFacts,
) -> Option<(String, &'a RegistryEffectFact)> {
    match &callee.kind {
        ExprKind::Path(path) => path_call_effect(path, scope, facts),
        ExprKind::Field { base, name } => {
            let receiver = type_fact_from_expr_with_registry(base, scope, facts);
            receiver_effect(&receiver, name, facts).map(|effect| (name.clone(), effect))
        }
        _ => None,
    }
}

fn path_call_effect<'a>(
    path: &[String],
    scope: &ExprFactScope,
    facts: &'a RegistryFacts,
) -> Option<(String, &'a RegistryEffectFact)> {
    let function_name = path.join(".");
    if let Some(effect) = facts.function_effect_fact(&function_name) {
        return Some((function_name, effect));
    }

    let (method, receiver_path) = path.split_last()?;
    if receiver_path.is_empty() {
        return None;
    }
    let receiver = scope.path_fact(receiver_path)?;
    receiver_effect(receiver, method, facts).map(|effect| (path.join("."), effect))
}

fn receiver_effect<'a>(
    receiver: &TypeFact,
    method: &str,
    facts: &'a RegistryFacts,
) -> Option<&'a RegistryEffectFact> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            facts.method_effect_fact(name, method)
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => facts.method_effect_fact(&format!("{name}.{variant}"), method),
        TypeFact::Trait { name } => facts.trait_method_effect_fact(name, method),
        _ => None,
    }
}
