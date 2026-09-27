mod incomplete;

use vela_analysis::{hints::type_fact_from_hint_with_schema, type_fact::TypeFact};
use vela_hir::module_graph::DeclarationKind;
use vela_hir::{
    binding::{LocalBinding, LocalBindingKind},
    type_hint::HirTypeHint,
};
use vela_syntax::ast::{AstNode, SyntaxTraitMethod};

use super::{Hover, HoverKind};
use crate::{
    DiagnosticRange, DisplayParts, LanguageServiceDatabases, QueryContext, SymbolRef,
    symbol_target::SymbolTarget,
};

pub(super) fn receiver_fact(
    db: &LanguageServiceDatabases,
    binding: &LocalBinding,
) -> Option<TypeFact> {
    if binding.kind != LocalBindingKind::Parameter
        || binding.name != "self"
        || binding.type_hint.is_some()
    {
        return None;
    }
    let graph = db.hir_db().graph();
    for declaration in graph.declarations().filter(|decl| {
        decl.span.source == binding.span.source && decl.span.contains(binding.span.start)
    }) {
        match declaration.kind {
            DeclarationKind::Trait => {
                return db
                    .graph_analysis_facts()
                    .declaration(declaration.id)
                    .cloned();
            }
            DeclarationKind::Impl => {
                let metadata = graph.impl_metadata(declaration.id)?;
                let hint = HirTypeHint {
                    path: metadata.target_path.clone(),
                    args: Vec::new(),
                    span: binding.span,
                };
                return Some(type_fact_from_hint_with_schema(
                    graph,
                    declaration.module,
                    &hint,
                    Some(db.schema_db().facts()),
                ));
            }
            _ => {}
        }
    }
    None
}

// Required trait methods have signature metadata but no executable HIR body or
// LocalBinding. Their header parameters still own static hover information.
pub(super) fn header_hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Hover> {
    if let Some(hover) = incomplete::hover(db, query, target, range) {
        return Some(hover);
    }
    let graph = db.hir_db().graph();
    let source = query.source_id()?;
    let offset = u32::try_from(target.range().start).ok()?;
    for method in query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxTraitMethod::cast)
    {
        if method.body().is_some() {
            continue;
        }
        for parameter in method.param_list()?.params() {
            let token = parameter.name_token()?;
            if usize::from(token.text_range().start()) != target.range().start
                || usize::from(token.text_range().end()) != target.range().end
            {
                continue;
            }
            let declaration = graph.declarations().find(|decl| {
                decl.kind == DeclarationKind::Trait
                    && decl.span.source == source
                    && decl.span.contains(offset)
            })?;
            let signature = &graph
                .trait_shape(declaration.id)?
                .methods
                .iter()
                .find(|entry| entry.name == method.name_text().as_deref().unwrap_or_default())?
                .signature;
            let param = signature
                .params
                .iter()
                .find(|param| param.name == target.text() && param.span.contains(offset))?;
            let fact = if param.name == "self" {
                db.graph_analysis_facts()
                    .declaration(declaration.id)
                    .cloned()
                    .unwrap_or(TypeFact::Unknown)
            } else {
                param.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                    type_fact_from_hint_with_schema(
                        graph,
                        declaration.module,
                        hint,
                        Some(db.schema_db().facts()),
                    )
                })
            };
            return Some(Hover::new(
                range,
                &param.name,
                HoverKind::Parameter,
                DisplayParts::type_name(fact.display_name()),
                None,
                Some(SymbolRef::local_at(
                    &param.name,
                    query.document_id().clone(),
                    target.range(),
                )),
            ));
        }
    }
    None
}
