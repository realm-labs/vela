use vela_hir::{
    module_graph::{DeclarationKind, Visibility},
    type_hint::EnumVariantFieldsHint,
};
use vela_syntax::{
    SyntaxToken,
    ast::{AstNode, SyntaxRecordExpr, SyntaxRecordPattern},
};

use crate::{LanguageServiceDatabases, QueryContext, symbol_target::SymbolTarget};

use super::{Definition, source_members::definition_from_named_span_with_symbol};

#[derive(Default)]
pub(super) struct RecordFieldNavigation {
    pub(super) definition: Option<Definition>,
    pub(super) type_definition: Option<Definition>,
}

impl LanguageServiceDatabases {
    pub(super) fn record_field_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<RecordFieldNavigation> {
        let tree = query.syntax_parse()?.tree();
        let matches = |token: SyntaxToken| {
            usize::from(token.text_range().start()) == target.range().start
                && usize::from(token.text_range().end()) == target.range().end
        };
        for node in tree.syntax().descendants() {
            let path = if let Some(record) = SyntaxRecordExpr::cast(node.clone()) {
                record
                    .fields()
                    .iter()
                    .any(|field| !field.is_shorthand() && field.label_token().is_some_and(&matches))
                    .then(|| record.path_segments())
            } else if let Some(pattern) = SyntaxRecordPattern::cast(node) {
                pattern
                    .fields()
                    .any(|field| !field.is_shorthand() && field.label_token().is_some_and(&matches))
                    .then(|| pattern.path_segments())
            } else {
                None
            };
            if let Some(path) = path {
                // A label owns a null result too; unresolved/missing fields
                // cannot borrow the enclosing constructor or a local value.
                return Some(
                    self.record_field_target(query, &path, target.text())
                        .unwrap_or_default(),
                );
            }
        }
        None
    }

    fn record_field_target(
        &self,
        query: &QueryContext<'_>,
        path: &[String],
        name: &str,
    ) -> Option<RecordFieldNavigation> {
        let graph = self.hir_db().graph();
        let module = graph.module_id(query.module_key()?)?;
        let source = graph
            .resolve_visible_declaration_path(module, path, DeclarationKind::Struct)
            .map(|declaration| (declaration, None))
            .or_else(|| {
                let (variant, owner) = path.split_last()?;
                graph
                    .resolve_visible_declaration_path(module, owner, DeclarationKind::Enum)
                    .map(|declaration| (declaration, Some(variant)))
            });
        if let Some((declaration, variant)) = source {
            if declaration.module != module && declaration.visibility != Visibility::Public {
                return None;
            }
            let (fields, owner) = if let Some(variant) = variant {
                let variant = graph
                    .enum_shape(declaration.id)?
                    .variants
                    .iter()
                    .find(|entry| entry.name == *variant)?;
                let EnumVariantFieldsHint::Record(fields) = &variant.fields else {
                    return None;
                };
                (
                    fields,
                    format!(
                        "{}::{}",
                        super::qualified_source_declaration_name(graph, declaration),
                        variant.name
                    ),
                )
            } else {
                (
                    &graph.struct_shape(declaration.id)?.fields,
                    super::qualified_source_declaration_name(graph, declaration),
                )
            };
            let field = fields.iter().find(|field| field.name == name)?;
            let fact = field.type_hint.as_ref().map(|hint| {
                crate::callable_context::query_type_fact_from_hint(
                    graph,
                    hint,
                    self.schema_db().facts(),
                )
            });
            return Some(RecordFieldNavigation {
                definition: definition_from_named_span_with_symbol(
                    self,
                    field.span,
                    &field.name,
                    Some(crate::symbol_ref::source_child_symbol(&owner, name)),
                ),
                type_definition: fact.and_then(|fact| self.type_definition_for_fact(&fact)),
            });
        }
        let owner = path.join("::");
        let fact = self.schema_db().facts().field_fact(&owner, name)?;
        Some(RecordFieldNavigation {
            definition: self
                .schema_db()
                .source_locations()
                .field_span(&owner, name)
                .and_then(|span| {
                    self.definition_from_span_with_symbol(
                        span,
                        Some(crate::symbol_ref::schema_member_symbol(&owner, name)),
                    )
                }),
            type_definition: self.type_definition_for_fact(fact),
        })
    }
}
