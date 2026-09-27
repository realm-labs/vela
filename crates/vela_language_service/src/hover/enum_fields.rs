use vela_hir::{module_graph::ModuleGraph, type_hint::EnumVariantFieldsHint};
use vela_syntax::ast::{AstNode, SyntaxParam, SyntaxStructField};

use super::{Hover, HoverKind, attr_docs};
use crate::{DiagnosticRange, DisplayParts, QueryContext, symbol_target::SymbolTarget};

pub(super) fn hover(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Hover> {
    // Metadata spans include hints and default expressions. Only the actual
    // declaration name token owns a field declaration hover.
    let is_name = query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .any(|node| {
            let token = SyntaxParam::cast(node.clone())
                .and_then(|field| field.name_token())
                .or_else(|| SyntaxStructField::cast(node).and_then(|field| field.name_token()));
            token.is_some_and(|token| {
                usize::from(token.text_range().start()) == target.range().start
                    && usize::from(token.text_range().end()) == target.range().end
            })
        });
    if !is_name {
        return None;
    }
    let source = query.source_id()?;
    let offset = u32::try_from(target.range().start).ok()?;
    for declaration in graph.declarations() {
        let Some(shape) = graph.enum_shape(declaration.id) else {
            continue;
        };
        for variant in &shape.variants {
            let (hint, docs) = match &variant.fields {
                EnumVariantFieldsHint::Tuple(fields) => {
                    let Some(field) = fields.iter().find(|field| {
                        field.span.source == source
                            && field.span.contains(offset)
                            && field.name == target.text()
                    }) else {
                        continue;
                    };
                    (field.type_hint.as_ref(), None)
                }
                EnumVariantFieldsHint::Record(fields) => {
                    let Some(field) = fields.iter().find(|field| {
                        field.span.source == source
                            && field.span.contains(offset)
                            && field.name == target.text()
                    }) else {
                        continue;
                    };
                    (field.type_hint.as_ref(), attr_docs(&field.attrs))
                }
                EnumVariantFieldsHint::Unit => continue,
            };
            let symbol = crate::symbol_ref::source_variant_field_symbol(
                graph,
                declaration.id,
                &variant.name,
                target.text(),
            )?;
            let label = format!(
                "{}::{}.{}",
                super::qualified_source_declaration_name(graph, declaration),
                variant.name,
                target.text()
            );
            return Some(Hover::new(
                range,
                label,
                HoverKind::Field,
                DisplayParts::type_name(
                    hint.map_or_else(|| "Any".to_owned(), |hint| hint.display()),
                ),
                docs,
                Some(symbol),
            ));
        }
    }
    None
}
