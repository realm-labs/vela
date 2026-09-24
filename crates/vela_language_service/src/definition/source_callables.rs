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
            && let Some(parameters) = self.schema_source_parameters_for_navigation(name)
        {
            return Some(parameters);
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

    fn schema_source_parameters_for_navigation(&self, name: &str) -> Option<SourceParameters<'_>> {
        let locations = self.schema_db().source_locations();
        let (span, terminal) = if let Some((owner, method)) = name.rsplit_once('.') {
            (
                locations
                    .method_span(owner, method)
                    .or_else(|| locations.trait_method_span(owner, method))?,
                method,
            )
        } else {
            (locations.function_span(name)?, name.rsplit("::").next()?)
        };
        let graph = self.hir_db().graph();
        for declaration in graph.declarations() {
            match declaration.kind {
                DeclarationKind::Function
                    if declaration.name_span == span && declaration.name == terminal =>
                {
                    let signature = graph.function_signature(declaration.id)?;
                    return Some(SourceParameters {
                        params: &signature.params,
                        declaration: Some(declaration.id),
                        variant: None,
                        required_method: None,
                    });
                }
                DeclarationKind::Impl => {
                    if let Some(method) = graph.impl_metadata(declaration.id).and_then(|metadata| {
                        metadata
                            .methods
                            .iter()
                            .find(|method| method.name_span == span && method.name == terminal)
                    }) {
                        return Some(SourceParameters {
                            params: &method.signature.params,
                            declaration: None,
                            variant: None,
                            required_method: None,
                        });
                    }
                }
                DeclarationKind::Trait => {
                    if let Some(method) = graph.trait_shape(declaration.id).and_then(|shape| {
                        shape
                            .methods
                            .iter()
                            .find(|method| method.name_span == span && method.name == terminal)
                    }) {
                        return Some(SourceParameters {
                            params: &method.signature.params,
                            declaration: None,
                            variant: None,
                            required_method: (!method.has_default).then_some(method.name_span),
                        });
                    }
                }
                _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentId, LineIndex, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
    };
    use serde_json::json;

    #[test]
    fn schema_method_source_spans_reach_impl_and_required_trait_parameters() {
        let document = DocumentId::from("/workspace/scripts/main.vela");
        let source = "struct Local {}\n\
impl Local { fn add(self, value: i64) -> i64 { return value; } }\n\
trait LocalTrait { fn add(self, amount: i64) -> i64; }\n\
fn main(boxed: host::Box, measured: host::Measure) { boxed.add(value = 1); measured.add(amount = 2); }\n";
        let files = [SourceFileSnapshot::new(document.clone(), source)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let mut db = LanguageServiceDatabases::new();
        db.update(&assemble_project_sources(
            &config,
            &files,
            &Workspace::new().snapshot(),
        ));
        let source_id = db.source_db().records()[&document].source_id().get();
        let impl_name = source.find("fn add(self, value").expect("impl name") + 3;
        let trait_name = source.find("fn add(self, amount").expect("trait name") + 3;
        let artifact = json!({
            "formatVersion": 1,
            "facts": {
                "types": [{"name":"host::Box","fact":{"kind":"host","name":"host::Box"}}],
                "traits": [{"name":"host::Measure","fact":{"kind":"trait","name":"host::Measure"}}],
                "methods": [{
                    "owner":"host::Box","name":"add",
                    "fact":{"kind":"function","params":[{"kind":"primitive","name":"i64"}],"returns":{"kind":"primitive","name":"i64"}},
                    "sourceSpan":{"source":source_id,"start":impl_name,"end":impl_name+3}
                }],
                "traitMethods": [{
                    "owner":"host::Measure","name":"add",
                    "fact":{"kind":"function","params":[{"kind":"primitive","name":"i64"}],"returns":{"kind":"primitive","name":"i64"}},
                    "sourceSpan":{"source":source_id,"start":trait_name,"end":trait_name+3}
                }]
            }
        });
        db.load_schema_artifact_json("/workspace/target/schema.json", &artifact.to_string());
        let lines = LineIndex::new(source);
        for (call, symbol, parameter, required) in [
            ("boxed.add(value", "host::Box.add", "value", false),
            ("measured.add(amount", "host::Measure.add", "amount", true),
        ] {
            let offset = source.find(call).expect("call") + call.find("add").expect("method");
            let callee = db
                .definition(&document, lines.position(offset))
                .expect("callee");
            assert_eq!(callee.symbol(), Some(&SymbolRef::Schema(symbol.into())));
            let parameters = db
                .source_parameters_for_navigation(&callee)
                .expect("source-backed schema parameters");
            assert_eq!(
                parameters
                    .params
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect::<Vec<_>>(),
                ["self", parameter]
            );
            assert_eq!(parameters.required_method.is_some(), required);
        }
    }
}
