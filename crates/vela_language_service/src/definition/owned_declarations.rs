use vela_common::Span;
use vela_hir::{
    module_graph::{Declaration, DeclarationKind},
    type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint, StructFieldHint},
};

use super::{Definition, source_members::definition_from_named_span_with_symbol};
use crate::{LanguageServiceDatabases, QueryContext, SymbolRef, symbol_target::SymbolTarget};

#[derive(Default)]
pub(super) struct DeclarationNavigation {
    pub(super) definition: Option<Definition>,
    pub(super) type_definition: Option<Definition>,
}

struct OwnedName<'a> {
    span: Span,
    name: &'a str,
    symbol: Option<SymbolRef>,
    hint: Option<&'a HirTypeHint>,
}

impl LanguageServiceDatabases {
    pub(super) fn source_declaration_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<DeclarationNavigation> {
        let source = query.source_id()?;
        let offset = u32::try_from(target.range().start).ok()?;
        let graph = self.hir_db().graph();
        for declaration in graph.declarations().filter(|declaration| {
            declaration.span.source == source && declaration.span.contains(offset)
        }) {
            if declaration.kind != DeclarationKind::Impl
                && let Some(mut navigation) = self.owned_name_navigation(
                    query,
                    target,
                    OwnedName {
                        span: declaration.name_span,
                        name: &declaration.name,
                        symbol: Some(super::source_symbol_for_declaration(graph, declaration)),
                        hint: None,
                    },
                )
            {
                navigation.type_definition = match declaration.kind {
                    DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait => {
                        navigation.definition.clone()
                    }
                    _ => self
                        .graph_analysis_facts()
                        .declaration(declaration.id)
                        .and_then(|fact| self.type_definition_for_fact(fact)),
                };
                return Some(navigation);
            }
            let navigation = match declaration.kind {
                DeclarationKind::Struct => self.owned_fields_navigation(
                    query,
                    target,
                    &graph.struct_shape(declaration.id)?.fields,
                    &super::qualified_source_declaration_name(graph, declaration),
                ),
                DeclarationKind::Enum => self.owned_variants_navigation(query, target, declaration),
                DeclarationKind::Function => self.owned_parameters_navigation(
                    query,
                    target,
                    graph.function_signature(declaration.id)?,
                    None,
                ),
                DeclarationKind::Trait | DeclarationKind::Impl => {
                    self.owned_methods_navigation(query, target, declaration)
                }
                _ => None,
            };
            if navigation.is_some() {
                return navigation;
            }
        }
        let bindings = query.bindings()?;
        let local =
            bindings.local_containing_source_range(target.range().start, target.range().end)?;
        let binding = bindings.local(local)?;
        let mut navigation = self.owned_name_navigation(
            query,
            target,
            OwnedName {
                span: binding.span,
                name: &binding.name,
                symbol: Some(self.definition_local_symbol_for_binding(binding)),
                hint: None,
            },
        )?;
        navigation.type_definition = self
            .schema_analysis_facts()
            .local(binding.id)
            .and_then(|fact| self.type_definition_for_fact(fact));
        Some(navigation)
    }

    fn owned_name_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
        name: OwnedName<'_>,
    ) -> Option<DeclarationNavigation> {
        if name.span.source != query.source_id()? || name.name != target.text() {
            return None;
        }
        let definition =
            definition_from_named_span_with_symbol(self, name.span, name.name, name.symbol)?;
        if definition.range() != super::diagnostic_range(query.text(), target.range()) {
            return None;
        }
        let type_definition = name
            .hint
            .map(|hint| {
                crate::callable_context::query_type_fact_from_hint(
                    self.hir_db().graph(),
                    hint,
                    self.schema_db().facts(),
                )
            })
            .and_then(|fact| self.type_definition_for_fact(&fact));
        Some(DeclarationNavigation {
            definition: Some(definition),
            type_definition,
        })
    }

    fn owned_fields_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
        fields: &[StructFieldHint],
        owner: &str,
    ) -> Option<DeclarationNavigation> {
        fields.iter().find_map(|field| {
            self.owned_name_navigation(
                query,
                target,
                OwnedName {
                    span: field.span,
                    name: &field.name,
                    symbol: Some(crate::symbol_ref::source_child_symbol(owner, &field.name)),
                    hint: field.type_hint.as_ref(),
                },
            )
        })
    }

    fn owned_variants_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
        declaration: &Declaration,
    ) -> Option<DeclarationNavigation> {
        let graph = self.hir_db().graph();
        for variant in &graph.enum_shape(declaration.id)?.variants {
            let owner = format!(
                "{}::{}",
                super::qualified_source_declaration_name(graph, declaration),
                variant.name
            );
            if let Some(mut navigation) = self.owned_name_navigation(
                query,
                target,
                OwnedName {
                    span: variant.span,
                    name: &variant.name,
                    symbol: crate::symbol_ref::source_enum_variant_symbol(
                        graph,
                        declaration.id,
                        &variant.name,
                    ),
                    hint: None,
                },
            ) {
                navigation.type_definition = self.definition_from_declaration(declaration);
                return Some(navigation);
            }
            let navigation = match &variant.fields {
                EnumVariantFieldsHint::Record(fields) => {
                    self.owned_fields_navigation(query, target, fields, &owner)
                }
                EnumVariantFieldsHint::Tuple(fields) => fields.iter().find_map(|field| {
                    self.owned_name_navigation(
                        query,
                        target,
                        OwnedName {
                            span: field.span,
                            name: &field.name,
                            symbol: crate::symbol_ref::source_variant_field_symbol(
                                graph,
                                declaration.id,
                                &variant.name,
                                &field.name,
                            ),
                            hint: field.type_hint.as_ref(),
                        },
                    )
                }),
                EnumVariantFieldsHint::Unit => None,
            };
            if navigation.is_some() {
                return navigation;
            }
        }
        None
    }

    fn owned_parameters_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
        signature: &FunctionSignature,
        receiver: Option<&Definition>,
    ) -> Option<DeclarationNavigation> {
        signature.params.iter().find_map(|parameter| {
            let source = self.source_record_for(parameter.span.source)?;
            let range = super::text_range_for_span(parameter.span)?;
            let mut navigation = self.owned_name_navigation(
                query,
                target,
                OwnedName {
                    span: parameter.span,
                    name: &parameter.name,
                    symbol: Some(SymbolRef::local_at(
                        &parameter.name,
                        source.document_id().clone(),
                        range,
                    )),
                    hint: parameter.type_hint.as_ref(),
                },
            )?;
            if parameter.name == "self" {
                navigation.type_definition = receiver.cloned();
            }
            Some(navigation)
        })
    }

    fn owned_methods_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
        declaration: &Declaration,
    ) -> Option<DeclarationNavigation> {
        let graph = self.hir_db().graph();
        let receiver = if declaration.kind == DeclarationKind::Trait {
            self.definition_from_declaration(declaration)
        } else {
            graph
                .impl_metadata(declaration.id)
                .and_then(|metadata| {
                    graph.resolve_visible_declaration_path(
                        declaration.module,
                        &metadata.target_path,
                        DeclarationKind::Struct,
                    )
                })
                .and_then(|declaration| self.definition_from_declaration(declaration))
        };
        let trait_methods = graph
            .trait_shape(declaration.id)
            .into_iter()
            .flat_map(|shape| &shape.methods)
            .map(|method| {
                (
                    method.name_span,
                    method.name.as_str(),
                    &method.signature,
                    crate::symbol_ref::source_member_symbol(graph, declaration.id, &method.name),
                )
            });
        let impl_methods = graph
            .impl_metadata(declaration.id)
            .into_iter()
            .flat_map(|metadata| &metadata.methods)
            .map(|method| {
                (
                    method.name_span,
                    method.name.as_str(),
                    &method.signature,
                    crate::symbol_ref::source_impl_method_symbol(
                        graph,
                        declaration.id,
                        &method.name,
                    ),
                )
            });
        for (span, name, signature, symbol) in trait_methods.chain(impl_methods) {
            if let Some(navigation) = self.owned_name_navigation(
                query,
                target,
                OwnedName {
                    span,
                    name,
                    symbol,
                    hint: None,
                },
            ) {
                return Some(navigation);
            }
            if let Some(navigation) =
                self.owned_parameters_navigation(query, target, signature, receiver.as_ref())
            {
                return Some(navigation);
            }
        }
        None
    }
}
