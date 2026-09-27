//! Retained function/impl header parameters have physical declarations even when
//! an unfinished header has no executable body or HIR LocalBinding.
use crate::hover::{Hover, HoverKind};
use crate::{
    DiagnosticRange, DisplayParts, LanguageServiceDatabases, QueryContext, SymbolRef,
    symbol_target::SymbolTarget,
};
use vela_analysis::{hints::type_fact_from_hint_with_schema, type_fact::TypeFact};
use vela_hir::{
    module_graph::DeclarationKind,
    type_hint::{HirTypeHint, lower_syntax_type_hint},
};
use vela_syntax::ast::{AstNode, SyntaxFunctionItem, SyntaxImplMethod, SyntaxParam};

pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Hover> {
    let graph = db.hir_db().graph();
    let source = query.source_id()?;
    let offset = u32::try_from(target.range().start).ok()?;
    for parameter in query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxParam::cast)
    {
        let Some(token) = parameter.name_token() else {
            continue;
        };
        if usize::from(token.text_range().start()) != target.range().start
            || usize::from(token.text_range().end()) != target.range().end
        {
            continue;
        }
        // A nested lambda/default's parameters are not this header's parameters.
        let owner = parameter.syntax().parent()?.parent()?;
        let (kind, name) = if let Some(function) = SyntaxFunctionItem::cast(owner.clone()) {
            if function.body().is_some() {
                continue;
            }
            let Some(name) = function.name_text() else {
                continue;
            };
            (DeclarationKind::Function, Some(name))
        } else if let Some(method) = SyntaxImplMethod::cast(owner) {
            if method.body().is_some() {
                continue;
            }
            (DeclarationKind::Impl, None)
        } else {
            continue;
        };
        let declaration = graph.declarations().find(|declaration| {
            declaration.kind == kind
                && declaration.span.source == source
                && declaration.span.contains(offset)
                && name.as_ref().is_none_or(|name| name == &declaration.name)
        })?;
        let hint = parameter
            .type_hint()
            .map(|hint| lower_syntax_type_hint(source, &hint))
            .or_else(|| {
                if kind != DeclarationKind::Impl || target.text() != "self" {
                    return None;
                }
                Some(HirTypeHint {
                    path: graph.impl_metadata(declaration.id)?.target_path.clone(),
                    args: Vec::new(),
                    span: declaration.span,
                })
            });
        let fact = hint.as_ref().map_or(TypeFact::Unknown, |hint| {
            type_fact_from_hint_with_schema(
                graph,
                declaration.module,
                hint,
                Some(db.schema_db().facts()),
            )
        });
        return Some(Hover::new(
            range,
            target.text(),
            HoverKind::Parameter,
            DisplayParts::type_name(fact.display_name()),
            None,
            Some(SymbolRef::local_at(
                target.text(),
                query.document_id().clone(),
                target.range(),
            )),
        ));
    }
    None
}
