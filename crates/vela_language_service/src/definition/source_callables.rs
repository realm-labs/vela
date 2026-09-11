use vela_common::Span;
use vela_hir::{ids::HirDeclId, module_graph::DeclarationKind, type_hint::FunctionSignature};

use crate::{Definition, LanguageServiceDatabases, SymbolRef};

impl LanguageServiceDatabases {
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
