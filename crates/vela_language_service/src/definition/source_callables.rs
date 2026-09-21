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
}

impl LanguageServiceDatabases {
    pub(crate) fn source_parameters_for_navigation(
        &self,
        callee: &Definition,
    ) -> Option<SourceParameters<'_>> {
        if let Some((signature, declaration)) = self.source_signature_for_navigation(callee) {
            return Some(SourceParameters {
                params: &signature.params,
                declaration,
                variant: None,
            });
        }
        let graph = self.hir_db().graph();
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
                    });
                }
            }
        }
        None
    }

    pub(super) fn source_signature_for_navigation(
        &self,
        callee: &Definition,
    ) -> Option<(&FunctionSignature, Option<HirDeclId>)> {
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
                    .map(|signature| (signature, Some(declaration.id))),
                DeclarationKind::Impl => graph
                    .impl_metadata(declaration.id)?
                    .methods
                    .iter()
                    .find(|method| matches(method.name_span))
                    .map(|method| (&method.signature, None)),
                DeclarationKind::Trait => graph
                    .trait_shape(declaration.id)?
                    .methods
                    .iter()
                    .find(|method| matches(method.name_span))
                    .map(|method| (&method.signature, None)),
                _ => None,
            })
    }
}
