use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxPathExpr},
};

use super::{Hover, HoverKind};
use crate::{
    DiagnosticRange, DisplayParts, LanguageServiceDatabases, QueryContext, SymbolRef,
    symbol_target::SymbolTarget,
};

// A qualified expression owns its result. Resolve the selected segment's
// canonical identity rather than the whole expression's lexical binding.
pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let qualified = query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxPathExpr::cast)
        .any(|path| {
            let tokens = path
                .path_tokens()
                .into_iter()
                .filter(|token| matches!(token.kind(), SyntaxKind::Ident | SyntaxKind::SelfKw))
                .collect::<Vec<_>>();
            tokens.len() > 1
                && tokens.iter().any(|token| {
                    usize::from(token.text_range().start()) == target.range().start
                        && usize::from(token.text_range().end()) == target.range().end
                })
        });
    if !qualified {
        return None;
    }
    let Some(symbol) = target.symbol() else {
        return Some(None);
    };
    Some(match symbol {
        SymbolRef::Source(_) => {
            let graph = db.hir_db().graph();
            let declaration = graph.declarations().find(|decl| {
                crate::symbol_ref::source_symbol_for_declaration(graph, decl) == *symbol
            });
            if let Some(declaration) = declaration {
                Some(super::hover_from_declaration(
                    graph,
                    db.graph_analysis_facts(),
                    declaration,
                    range,
                ))
            } else if let Some(key) = graph
                .module_ids()
                .filter_map(|id| graph.module_key(id))
                .find(|key| crate::symbol_ref::source_module_symbol(key) == *symbol)
            {
                let label = key.path.join();
                Some(Hover::new(
                    range,
                    &label,
                    HoverKind::Module,
                    DisplayParts::keyword_symbol("module", &label),
                    None,
                    target.symbol().cloned(),
                ))
            } else {
                graph.declarations().find_map(|decl| {
                    graph
                        .enum_shape(decl.id)?
                        .variants
                        .iter()
                        .find(|variant| {
                            crate::symbol_ref::source_enum_variant_symbol(
                                graph,
                                decl.id,
                                &variant.name,
                            )
                            .as_ref()
                                == target.symbol()
                        })
                        .map(|variant| super::enum_variant_hover(graph, decl, variant, range))
                })
            }
        }
        SymbolRef::Schema(name) => super::schema::symbol_hover(db.schema_db().facts(), name, range),
        SymbolRef::Builtin(name) => super::stdlib_function_hover(name, range),
        SymbolRef::Local(_) => None,
    })
}
