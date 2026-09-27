//! Type-hint tokens own hover, including an unresolved result. Resolution stays
//! in the requesting module; values and unrelated same-named types cannot lend
//! metadata. Builtin conversion is shared with HIR and analysis.
use vela_analysis::{hints, type_fact::TypeFact};
use vela_hir::{
    module_graph::{DeclarationKind, Visibility},
    type_hint::lower_syntax_type_hint,
};
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxTypeHint},
};

use super::{Hover, HoverKind};
use crate::{
    DiagnosticRange, DisplayParts, LanguageServiceDatabases, QueryContext, SymbolRef,
    symbol_target::SymbolTarget,
};

pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    for hint in query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxTypeHint::cast)
    {
        let tokens = hint
            .path_tokens()
            .into_iter()
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .collect::<Vec<_>>();
        let Some(index) = tokens.iter().position(|token| {
            usize::from(token.text_range().start()) == target.range().start
                && usize::from(token.text_range().end()) == target.range().end
        }) else {
            continue;
        };
        let graph = db.hir_db().graph();
        let module = graph.module_id(query.module_key()?)?;
        if index + 1 != tokens.len() {
            let path = tokens[..=index]
                .iter()
                .map(|token| token.text().to_owned())
                .collect::<Vec<_>>();
            return Some(
                graph
                    .expand_import_path(module, &path)
                    .and_then(|path| super::imports::path_hover(db, query, &path, range))
                    .filter(|hover| {
                        matches!(
                            hover.kind(),
                            HoverKind::Module | HoverKind::Type | HoverKind::Trait
                        )
                    }),
            );
        }
        let hir = lower_syntax_type_hint(query.source_id()?, &hint);
        if (hir.path.len() == 1 && hints::builtin_type_fact(&hir.path[0]).is_some())
            || (hir.path.as_slice() == ["task", "Error"] && hir.args.is_empty())
        {
            let fact = hints::type_fact_from_hint_with_schema(
                graph,
                module,
                &hir,
                Some(db.schema_db().facts()),
            );
            return Some(Some(Hover::new(
                range,
                hir.path.join("::"),
                HoverKind::Type,
                DisplayParts::type_name(fact.display_name()),
                None,
                Some(SymbolRef::Builtin(hir.path.join("::"))),
            )));
        }
        let path = graph.expand_import_path(module, &hir.path);
        let resolved = path.as_ref().and_then(|path| {
            [
                DeclarationKind::Struct,
                DeclarationKind::Enum,
                DeclarationKind::Trait,
                DeclarationKind::Function,
                DeclarationKind::Const,
                DeclarationKind::State,
            ]
            .into_iter()
            .find_map(|kind| graph.declaration_by_type_path(path, query.module_key()?, kind))
        });
        if let Some(declaration) = resolved {
            if hir.args.is_empty()
                && matches!(
                    declaration.kind,
                    DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
                )
                && (declaration.module == module || declaration.visibility == Visibility::Public)
            {
                return Some(Some(super::hover_from_declaration(
                    graph,
                    db.graph_analysis_facts(),
                    declaration,
                    range,
                )));
            }
        } else if hir.args.is_empty()
            && let Some(path) = &path
        {
            let name = path.join("::");
            let schema = db.schema_db().facts();
            if schema.type_fact(&name).is_some() || schema.trait_fact(&name).is_some() {
                return Some(super::schema::symbol_hover(schema, &name, range));
            }
        }
        return Some(Some(Hover::new(
            range,
            target.text(),
            HoverKind::Type,
            DisplayParts::type_name(TypeFact::Any.display_name()),
            None,
            None,
        )));
    }
    None
}
