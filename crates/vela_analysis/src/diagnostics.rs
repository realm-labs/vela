mod candidates;
pub mod effects;
pub mod match_exhaustiveness;
pub mod match_patterns;
pub mod member;

use vela_common::Diagnostic;
use vela_syntax::ast::{Expr, ExprKind, FunctionItem, ImplItem, ItemKind, SourceFile, TraitItem};

use crate::expression::{ExprFactScope, type_fact_from_syntax_hint};
use crate::registry::RegistryFacts;

#[must_use]
pub fn source_diagnostics(source: &SourceFile, facts: &RegistryFacts) -> Vec<Diagnostic> {
    source
        .items
        .iter()
        .flat_map(|item| match &item.kind {
            ItemKind::Function(function) => function_diagnostics(function, facts),
            ItemKind::Trait(item) => trait_diagnostics(item, facts),
            ItemKind::Impl(item) => impl_diagnostics(item, facts),
            ItemKind::Use(_)
            | ItemKind::Const(_)
            | ItemKind::Global(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_) => Vec::new(),
        })
        .collect()
}

fn trait_diagnostics(item: &TraitItem, facts: &RegistryFacts) -> Vec<Diagnostic> {
    item.methods
        .iter()
        .filter_map(|method| {
            let body = method.default_body.as_ref()?;
            Some(function_body_diagnostics(&method.params, body, facts))
        })
        .flatten()
        .collect()
}

fn impl_diagnostics(item: &ImplItem, facts: &RegistryFacts) -> Vec<Diagnostic> {
    item.methods
        .iter()
        .flat_map(|method| function_diagnostics(&method.function, facts))
        .collect()
}

fn function_diagnostics(function: &FunctionItem, facts: &RegistryFacts) -> Vec<Diagnostic> {
    function_body_diagnostics(&function.params, &function.body, facts)
}

fn function_body_diagnostics(
    params: &[vela_syntax::ast::Param],
    body: &vela_syntax::ast::Block,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    let mut scope = ExprFactScope::new();
    for param in params {
        if let Some(hint) = &param.type_hint {
            scope.insert_path([param.name.clone()], type_fact_from_syntax_hint(hint));
        }
    }
    let expr = Expr {
        kind: ExprKind::Block(body.clone()),
        span: body.span,
    };
    let mut diagnostics = member::member_access_diagnostics(&expr, &scope, facts);
    diagnostics.extend(match_patterns::match_pattern_diagnostics(
        &expr, &scope, facts,
    ));
    diagnostics.extend(match_exhaustiveness::match_exhaustiveness_diagnostics(
        &expr, &scope, facts,
    ));
    diagnostics
}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_syntax::parser::parse_source;

    use super::*;

    #[test]
    fn source_diagnostics_collect_function_member_errors() {
        let source = parse_source(
            SourceId::new(1),
            "pub fn main(scores: Array<i64>) { return scores.frist() }",
        );

        let diagnostics = source_diagnostics(&source, &RegistryFacts::default());

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("analysis::unknown_method")
                && diagnostic.span.is_some()
                && diagnostic
                    .labels
                    .iter()
                    .any(|label| label.message == "did you mean `first`?")
        }));
    }

    #[test]
    fn source_diagnostics_degrade_unknown_schema_receivers_to_any() {
        let source = parse_source(
            SourceId::new(1),
            "pub fn main(player: Player, scores: Array<i64>) {
                player.level
                scores.frist()
            }",
        );

        let diagnostics = source_diagnostics(&source, &RegistryFacts::default());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("analysis::unknown_method")
            })
        );
        assert!(
            diagnostics.iter().all(|diagnostic| {
                diagnostic.code.as_deref() != Some("analysis::unknown_field")
            })
        );
    }
}
