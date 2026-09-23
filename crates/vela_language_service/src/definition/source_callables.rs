use vela_common::Span;
use vela_hir::{
    ids::HirDeclId,
    module_graph::DeclarationKind,
    type_hint::{EnumVariantFieldsHint, FunctionSignature, ParamHint},
};

use crate::{Definition, LanguageServiceDatabases, SymbolRef};

pub(crate) struct SourceParameters<'a> {
    pub(crate) params: &'a [ParamHint],
    pub(crate) declaration: Option<HirDeclId>,
    pub(crate) variant: Option<(HirDeclId, &'a str)>,
    pub(crate) required_method: Option<Span>,
}

impl LanguageServiceDatabases {
    pub(crate) fn source_parameters_for_navigation(
        &self,
        callee: &Definition,
    ) -> Option<SourceParameters<'_>> {
        if let Some((signature, declaration, required_method)) =
            self.source_signature_for_navigation(callee)
        {
            return Some(SourceParameters {
                params: &signature.params,
                declaration,
                variant: None,
                required_method,
            });
        }
        let graph = self.hir_db().graph();
        if let Some(SymbolRef::Schema(name)) = callee.symbol()
            && let Some(span) = self.schema_db().source_locations().function_span(name)
            && let Some(declaration) = graph.declarations().find(|declaration| {
                declaration.kind == DeclarationKind::Function
                    && declaration.name_span == span
                    && name.rsplit("::").next() == Some(declaration.name.as_str())
            })
            && let Some(signature) = graph.function_signature(declaration.id)
        {
            return Some(SourceParameters {
                params: &signature.params,
                declaration: Some(declaration.id),
                variant: None,
                required_method: None,
            });
        }
        for declaration in graph.declarations() {
            let Some(shape) = graph.enum_shape(declaration.id) else {
                continue;
            };
            for variant in &shape.variants {
                let EnumVariantFieldsHint::Tuple(parameters) = &variant.fields else {
                    continue;
                };
                let candidate = super::source_members::definition_from_named_span_with_symbol(
                    self,
                    variant.span,
                    &variant.name,
                    crate::symbol_ref::source_enum_variant_symbol(
                        graph,
                        declaration.id,
                        &variant.name,
                    ),
                );
                if candidate.as_ref() == Some(callee) {
                    return Some(SourceParameters {
                        params: parameters,
                        declaration: None,
                        variant: Some((declaration.id, &variant.name)),
                        required_method: None,
                    });
                }
            }
        }
        None
    }

    pub(super) fn source_signature_for_navigation(
        &self,
        callee: &Definition,
    ) -> Option<(&FunctionSignature, Option<HirDeclId>, Option<Span>)> {
        if !matches!(callee.symbol(), Some(SymbolRef::Source(_))) {
            return None;
        }
        let graph = self.hir_db().graph();
        // Use the resolved target location, not its short name or an owner
        // string that could collide across trait implementations/modules.
        let matches = |span: Span| {
            self.definition_from_span_with_symbol(span, None)
                .is_some_and(|candidate| {
                    candidate.document_id() == callee.document_id()
                        && candidate.range() == callee.range()
                })
        };
        graph
            .declarations()
            .find_map(|declaration| match declaration.kind {
                DeclarationKind::Function if matches(declaration.name_span) => graph
                    .function_signature(declaration.id)
                    .map(|signature| (signature, Some(declaration.id), None)),
                DeclarationKind::Impl => graph
                    .impl_metadata(declaration.id)?
                    .methods
                    .iter()
                    .find(|method| matches(method.name_span))
                    .map(|method| (&method.signature, None, None)),
                DeclarationKind::Trait => graph
                    .trait_shape(declaration.id)?
                    .methods
                    .iter()
                    .find(|method| matches(method.name_span))
                    .map(|method| {
                        (
                            &method.signature,
                            None,
                            (!method.has_default).then_some(method.name_span),
                        )
                    }),
                _ => None,
            })
    }
}
