use vela_analysis::type_fact::TypeFact;

use super::{Hover, HoverKind};
use crate::{
    DiagnosticRange, DisplayParts, LanguageServiceDatabases, QueryContext,
    callable_context::CallableOrigin, symbol_target::SymbolTarget,
};

pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let call = query.syntax_call()?;
    if !call.arguments().iter().any(|argument| {
        argument.name_token().is_some_and(|token| {
            usize::from(token.text_range().start()) == target.range().start
                && usize::from(token.text_range().end()) == target.range().end
        })
    }) {
        return None;
    }
    let callables = query.call_target_facts(db);
    if callables.is_empty()
        || !callables.iter().all(|callable| {
            callable.supports_named_arguments()
                && callable
                    .params()
                    .iter()
                    .any(|param| param.name() == target.text())
        })
    {
        return Some(None);
    }
    if let Some((_, binding)) =
        crate::named_argument_sites::target(db, query.document_id(), target.range())
    {
        return Some(Some(super::local_hover(
            db,
            binding,
            super::local_fact(binding, db.schema_analysis_facts()).unwrap_or(TypeFact::Unknown),
            range,
            None,
        )));
    }
    if let Some(parameter) = crate::signature_parameters::target(db, query) {
        let fact = parameter
            .parameter
            .type_hint
            .as_ref()
            .map_or(TypeFact::Unknown, |hint| {
                crate::callable_context::query_type_fact_from_hint(
                    db.hir_db().graph(),
                    hint,
                    db.schema_db().facts(),
                )
            });
        return Some(Some(Hover::new(
            range,
            target.text(),
            HoverKind::Parameter,
            DisplayParts::type_name(fact.display_name()),
            None,
            parameter.symbol(db),
        )));
    }
    // Metadata-only parameters have types and names, but no source local ID.
    // Missing source ownership may not borrow such a parameter identity.
    if callables.iter().any(|callable| {
        matches!(
            callable.origin(),
            CallableOrigin::Source | CallableOrigin::SourceMethod | CallableOrigin::SourceVariant
        )
    }) {
        return Some(None);
    }
    let mut facts = callables.iter().filter_map(|callable| {
        callable
            .params()
            .iter()
            .find(|param| param.name() == target.text())
            .map(|param| param.type_fact())
    });
    let fact = facts.next()?;
    Some(facts.all(|alternative| alternative == fact).then(|| {
        Hover::new(
            range,
            target.text(),
            HoverKind::Parameter,
            DisplayParts::type_name(fact.display_name()),
            None,
            None,
        )
    }))
}
