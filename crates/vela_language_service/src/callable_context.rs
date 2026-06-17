use vela_analysis::facts::AnalysisFacts;
use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::stdlib::{
    LambdaFact, StdlibFunctionFact, StdlibMethodFact, stdlib_function_completion_facts,
    stdlib_method_fact_with_lambda_arity,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint};

use crate::query_context::type_fact_for_source_range;
use crate::{LanguageServiceDatabases, TextRange};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CallableOrigin {
    Source,
    SourceMethod,
    SourceVariant,
    Schema,
    SchemaMethod,
    Stdlib,
    StdlibMethod,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CallableFacts {
    name: String,
    params: Vec<CallableParameterFacts>,
    returns: TypeFact,
    origin: CallableOrigin,
}

impl CallableFacts {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn params(&self) -> &[CallableParameterFacts] {
        &self.params
    }

    #[must_use]
    pub const fn returns(&self) -> &TypeFact {
        &self.returns
    }

    #[must_use]
    pub const fn origin(&self) -> CallableOrigin {
        self.origin
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CallableParameterFacts {
    name: String,
    type_fact: TypeFact,
    defaulted: bool,
}

impl CallableParameterFacts {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn type_fact(&self) -> &TypeFact {
        &self.type_fact
    }

    #[must_use]
    pub const fn defaulted(&self) -> bool {
        self.defaulted
    }
}

pub(crate) fn source_callable_facts(
    databases: &LanguageServiceDatabases,
    callee: &str,
) -> Vec<CallableFacts> {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    let schema = databases.schema_db().facts();
    graph
        .declarations()
        .filter(|declaration| {
            declaration.kind == DeclarationKind::Function
                && (declaration.name == callee
                    || qualified_declaration_label(graph, declaration.id) == callee)
        })
        .filter_map(|declaration| {
            let signature = graph.function_signature(declaration.id)?;
            let inferred = facts.declaration(declaration.id);
            let inferred_params = match inferred {
                Some(TypeFact::Function { params, .. }) => params.as_slice(),
                _ => &[],
            };
            let inferred_returns = match inferred {
                Some(TypeFact::Function { returns, .. }) => Some(returns),
                _ => None,
            };
            let params = signature
                .params
                .iter()
                .enumerate()
                .map(|(index, param)| {
                    let type_fact = inferred_params
                        .get(index)
                        .cloned()
                        .filter(|fact| !matches!(fact, TypeFact::Unknown))
                        .or_else(|| {
                            param
                                .type_hint
                                .as_ref()
                                .map(|hint| query_type_fact_from_hint(graph, hint, schema))
                        })
                        .unwrap_or(TypeFact::Unknown);
                    CallableParameterFacts {
                        name: param.name.clone(),
                        type_fact,
                        defaulted: param.default_value_span.is_some(),
                    }
                })
                .collect::<Vec<_>>();
            let returns = match inferred_returns {
                Some(fact) if !matches!(fact.as_ref(), TypeFact::Unknown) => fact.as_ref().clone(),
                _ => signature
                    .return_type
                    .as_ref()
                    .map(|hint| query_type_fact_from_hint(graph, hint, schema))
                    .unwrap_or(TypeFact::Unknown),
            };
            Some(CallableFacts {
                name: declaration.name.clone(),
                params,
                returns,
                origin: CallableOrigin::Source,
            })
        })
        .collect()
}

pub(crate) fn callable_facts(
    databases: &LanguageServiceDatabases,
    callee: &str,
) -> Vec<CallableFacts> {
    let mut facts = source_callable_facts(databases, callee);
    facts.extend(source_variant_callable_facts(databases, callee));
    facts.extend(schema_callable_facts(databases.schema_db().facts(), callee));
    facts.extend(stdlib_callable_facts(callee));
    facts
}

pub(crate) fn member_callable_facts(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    receiver_range: TextRange,
    method: &str,
    args_prefix: &str,
) -> Vec<CallableFacts> {
    if method.is_empty() {
        return Vec::new();
    }
    let Some(receiver) = type_fact_for_source_range(databases, source_id, receiver_range) else {
        return Vec::new();
    };
    let mut facts = source_method_callable_facts(databases, &receiver, method);
    facts.extend(schema_method_callable_facts(
        databases.schema_db().facts(),
        &receiver,
        method,
    ));
    facts.extend(stdlib_method_callable_facts(&receiver, method, args_prefix));
    facts
}

fn source_method_callable_facts(
    databases: &LanguageServiceDatabases,
    receiver: &TypeFact,
    method: &str,
) -> Vec<CallableFacts> {
    let graph = databases.hir_db().graph();
    let schema = databases.schema_db().facts();
    let owner_names = record_owner_names(receiver);
    graph
        .declarations()
        .filter_map(|declaration| {
            if declaration.kind != DeclarationKind::Impl {
                return None;
            }
            let metadata = graph.impl_metadata(declaration.id)?;
            if !matches!(
                metadata.kind,
                vela_hir::type_hint::ImplMetadataKind::Inherent
            ) {
                return None;
            }
            let matches_owner = owner_names.iter().any(|owner| {
                metadata
                    .target_path
                    .last()
                    .is_some_and(|name| name == owner)
                    || metadata.target_path.join("::") == *owner
            });
            if !matches_owner {
                return None;
            }
            let method = metadata.methods.iter().find(|entry| entry.name == method)?;
            let owner = metadata.target_path.join("::");
            Some(callable_facts_from_signature(
                graph,
                schema,
                format!("{owner}.{}", method.name),
                &method.signature,
                CallableOrigin::SourceMethod,
                true,
            ))
        })
        .collect()
}

fn schema_method_callable_facts(
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Vec<CallableFacts> {
    let Some((owner, fact)) = schema_method_fact_for_receiver(schema, receiver, method) else {
        return Vec::new();
    };
    let TypeFact::Function { params, returns } = fact else {
        return Vec::new();
    };
    vec![CallableFacts {
        name: format!("{owner}.{method}"),
        params: indexed_callable_parameters(params.clone()),
        returns: returns.as_ref().clone(),
        origin: CallableOrigin::SchemaMethod,
    }]
}

fn stdlib_method_callable_facts(
    receiver: &TypeFact,
    method: &str,
    args_prefix: &str,
) -> Vec<CallableFacts> {
    let lambda_param_count = first_lambda_param_count(args_prefix);
    let Some(fact) =
        stdlib_method_fact_with_lambda_arity(receiver, method, None, lambda_param_count)
    else {
        return Vec::new();
    };
    vec![stdlib_method_callable_fact(fact)]
}

fn source_variant_callable_facts(
    databases: &LanguageServiceDatabases,
    callee: &str,
) -> Vec<CallableFacts> {
    let graph = databases.hir_db().graph();
    let schema = databases.schema_db().facts();
    graph
        .declarations()
        .filter(|declaration| declaration.kind == DeclarationKind::Enum)
        .filter_map(|declaration| {
            let owner = qualified_declaration_label(graph, declaration.id);
            let shape = graph.enum_shape(declaration.id)?;
            Some((declaration, owner, shape))
        })
        .flat_map(|(declaration, owner, shape)| {
            shape.variants.iter().filter_map(move |variant| {
                if !variant_callable_name_matches(
                    callee,
                    declaration.name.as_str(),
                    &owner,
                    &variant.name,
                ) {
                    return None;
                }
                let EnumVariantFieldsHint::Tuple(fields) = &variant.fields else {
                    return None;
                };
                let params = fields
                    .iter()
                    .map(|field| CallableParameterFacts {
                        name: field.name.clone(),
                        type_fact: field.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                            query_type_fact_from_hint(graph, hint, schema)
                        }),
                        defaulted: false,
                    })
                    .collect::<Vec<_>>();
                Some(CallableFacts {
                    name: format!("{owner}::{}", variant.name),
                    params,
                    returns: TypeFact::enum_type(&owner, Some(&variant.name)),
                    origin: CallableOrigin::SourceVariant,
                })
            })
        })
        .collect()
}

fn schema_callable_facts(schema: &RegistryFacts, callee: &str) -> Vec<CallableFacts> {
    schema
        .functions()
        .filter(|function| callable_name_matches(&function.name, callee))
        .filter_map(|function| {
            let TypeFact::Function { params, returns } = function.fact else {
                return None;
            };
            Some(CallableFacts {
                name: function.name.clone(),
                params: indexed_callable_parameters(params),
                returns: *returns,
                origin: CallableOrigin::Schema,
            })
        })
        .collect()
}

fn stdlib_callable_facts(callee: &str) -> Vec<CallableFacts> {
    stdlib_function_completion_facts()
        .into_iter()
        .filter(|fact| callable_name_matches(fact.name, callee))
        .map(stdlib_callable_fact)
        .collect()
}

fn stdlib_callable_fact(fact: StdlibFunctionFact) -> CallableFacts {
    CallableFacts {
        name: fact.name.to_owned(),
        params: indexed_callable_parameters(fact.params),
        returns: fact.returns,
        origin: CallableOrigin::Stdlib,
    }
}

fn stdlib_method_callable_fact(fact: StdlibMethodFact) -> CallableFacts {
    let params = fact
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| CallableParameterFacts {
            name: if is_lambda_parameter(param, fact.lambda.as_ref()) {
                "callback".to_owned()
            } else {
                format!("arg{index}")
            },
            type_fact: param.clone(),
            defaulted: false,
        })
        .collect();
    CallableFacts {
        name: format!("{}.{}", fact.receiver.display_name(), fact.method),
        params,
        returns: fact.returns,
        origin: CallableOrigin::StdlibMethod,
    }
}

fn callable_facts_from_signature(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    name: String,
    signature: &vela_hir::type_hint::FunctionSignature,
    origin: CallableOrigin,
    skip_self: bool,
) -> CallableFacts {
    let params = signature
        .params
        .iter()
        .filter(|param| !skip_self || param.name != "self")
        .map(|param| CallableParameterFacts {
            name: param.name.clone(),
            type_fact: param.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                query_type_fact_from_hint(graph, hint, schema)
            }),
            defaulted: param.default_value_span.is_some(),
        })
        .collect();
    let returns = signature
        .return_type
        .as_ref()
        .map_or(TypeFact::Unknown, |hint| {
            query_type_fact_from_hint(graph, hint, schema)
        });
    CallableFacts {
        name,
        params,
        returns,
        origin,
    }
}

fn indexed_callable_parameters(params: Vec<TypeFact>) -> Vec<CallableParameterFacts> {
    params
        .into_iter()
        .enumerate()
        .map(|(index, type_fact)| CallableParameterFacts {
            name: format!("arg{index}"),
            type_fact,
            defaulted: false,
        })
        .collect()
}

fn callable_name_matches(name: &str, callee: &str) -> bool {
    name == callee
        || name
            .rsplit("::")
            .next()
            .is_some_and(|segment| segment == callee)
}

fn variant_callable_name_matches(
    callee: &str,
    enum_name: &str,
    owner: &str,
    variant: &str,
) -> bool {
    callee == variant
        || callee == format!("{enum_name}::{variant}")
        || callee == format!("{owner}::{variant}")
}

fn schema_method_fact_for_receiver<'a>(
    schema: &'a RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<(String, &'a TypeFact)> {
    owner_names(receiver).into_iter().find_map(|owner| {
        schema
            .method_fact(&owner, method)
            .or_else(|| schema.trait_method_fact(&owner, method))
            .map(|fact| (owner, fact))
    })
}

fn owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = record_owner_names(receiver);
    if let TypeFact::Host { name } | TypeFact::Trait { name } = receiver {
        push_owner_name(&mut owners, name);
        if let Some(short) = name.rsplit("::").next()
            && short != name
        {
            push_owner_name(&mut owners, short);
        }
    }
    owners
}

fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_record_owner_names(receiver, &mut owners);
    owners
}

fn collect_record_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Record { name } => {
            push_owner_name(owners, name);
            if let Some(short) = name.rsplit("::").next()
                && short != name
            {
                push_owner_name(owners, short);
            }
        }
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_record_owner_names(fact, owners);
            }
        }
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
}

fn first_lambda_param_count(args_text: &str) -> Option<usize> {
    let start = args_text.find('|')?;
    let rest = &args_text[start + 1..];
    let end = rest.find('|')?;
    let params = rest[..end].trim();
    if params.is_empty() {
        Some(0)
    } else {
        Some(
            params
                .split(',')
                .filter(|param| !param.trim().is_empty())
                .count(),
        )
    }
}

fn is_lambda_parameter(param: &TypeFact, lambda: Option<&LambdaFact>) -> bool {
    let Some(lambda) = lambda else {
        return false;
    };
    param == &TypeFact::function(lambda.params.clone(), lambda.returns.clone())
}

fn query_type_fact_from_hint(
    graph: &ModuleGraph,
    hint: &HirTypeHint,
    schema: &RegistryFacts,
) -> TypeFact {
    let fact = type_fact_from_hint(graph, hint);
    if matches!(fact, TypeFact::Unknown) {
        schema_fact_for_hint(hint, schema).unwrap_or(TypeFact::Unknown)
    } else {
        fact
    }
}

fn schema_fact_for_hint(hint: &HirTypeHint, schema: &RegistryFacts) -> Option<TypeFact> {
    if !hint.args.is_empty() {
        return None;
    }
    let qualified = hint.path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| schema.trait_fact(&qualified))
        .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
        .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
        .cloned()
}

fn qualified_declaration_label(
    graph: &ModuleGraph,
    declaration: vela_hir::ids::HirDeclId,
) -> String {
    let Some(declaration) = graph.declaration(declaration) else {
        return String::new();
    };
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    let module = module_path.join();
    if module.is_empty() {
        declaration.name.clone()
    } else {
        format!("{module}::{}", declaration.name)
    }
}
