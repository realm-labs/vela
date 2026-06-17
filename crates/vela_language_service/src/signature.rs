use vela_analysis::facts::AnalysisFacts;
use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::stdlib::{
    LambdaFact, StdlibFunctionFact, StdlibMethodFact, stdlib_function_completion_facts,
    stdlib_method_fact_with_lambda_arity,
};
use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint, ImplMetadataKind};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext, TextRange};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignatureHelp {
    active_signature: usize,
    active_parameter: usize,
    signatures: Vec<SignatureInformation>,
}

impl SignatureHelp {
    #[must_use]
    pub const fn active_signature(&self) -> usize {
        self.active_signature
    }

    #[must_use]
    pub const fn active_parameter(&self) -> usize {
        self.active_parameter
    }

    #[must_use]
    pub fn signatures(&self) -> &[SignatureInformation] {
        &self.signatures
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignatureInformation {
    label: String,
    parameters: Vec<SignatureParameter>,
}

impl SignatureInformation {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn parameters(&self) -> &[SignatureParameter] {
        &self.parameters
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignatureParameter {
    label: String,
    type_fact: TypeFact,
}

impl SignatureParameter {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn type_fact(&self) -> &TypeFact {
        &self.type_fact
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CallContext {
    callee: String,
    callee_range: TextRange,
    args_prefix: String,
    active_parameter: usize,
}

struct MemberCallLookup<'a> {
    graph: &'a ModuleGraph,
    facts: &'a AnalysisFacts,
    text: &'a str,
    source_id: SourceId,
    bindings: &'a BindingMap,
    method: &'a str,
    method_range: TextRange,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn signature_help(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<SignatureHelp> {
        let query = QueryContext::from_databases(self, document_id, position)?;
        let source_id = query.source_id()?;
        let context = call_context_at(query.text(), query.position())?;
        let signatures = self.signature_candidates_for_context(source_id, query.text(), &context);
        if signatures.is_empty() {
            return None;
        }
        let max_parameter = signatures[0].parameters.len().saturating_sub(1);
        Some(SignatureHelp {
            active_signature: 0,
            active_parameter: context.active_parameter.min(max_parameter),
            signatures,
        })
    }

    pub(crate) fn signature_candidates(&self, callee: &str) -> Vec<SignatureInformation> {
        let mut signatures = self.script_signatures(callee);
        signatures.extend(self.script_variant_signatures(callee));
        signatures.extend(self.schema_signatures(callee));
        signatures.extend(stdlib_function_signatures(callee));
        signatures
    }

    pub(crate) fn signature_candidates_for_callee_range(
        &self,
        source_id: SourceId,
        text: &str,
        callee: String,
        callee_range: TextRange,
        args_prefix: String,
    ) -> Vec<SignatureInformation> {
        let context = CallContext {
            callee,
            callee_range,
            args_prefix,
            active_parameter: 0,
        };
        self.signature_candidates_for_context(source_id, text, &context)
    }

    fn signature_candidates_for_context(
        &self,
        source_id: SourceId,
        text: &str,
        context: &CallContext,
    ) -> Vec<SignatureInformation> {
        if let Some(signatures) = self.member_signatures(source_id, text, context)
            && !signatures.is_empty()
        {
            return signatures;
        }
        self.signature_candidates(&context.callee)
    }

    fn member_signatures(
        &self,
        source_id: SourceId,
        text: &str,
        context: &CallContext,
    ) -> Option<Vec<SignatureInformation>> {
        let (_receiver, method) = context.callee.rsplit_once('.')?;
        if method.is_empty() {
            return None;
        }
        let method_range = TextRange::new(
            context.callee_range.end.saturating_sub(method.len()),
            context.callee_range.end,
        );
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        self.bindings_at(source_id, method_range.start)
            .find_map(|bindings| {
                let lookup = MemberCallLookup {
                    graph,
                    facts: &facts,
                    text,
                    source_id,
                    bindings,
                    method,
                    method_range,
                };
                let mut signatures = self.script_method_signatures(&lookup);
                signatures.extend(self.schema_method_signatures(&lookup));
                signatures.extend(self.stdlib_method_signatures(&lookup, &context.args_prefix));
                (!signatures.is_empty()).then_some(signatures)
            })
    }

    fn bindings_at<'a>(
        &'a self,
        source_id: SourceId,
        offset: usize,
    ) -> impl Iterator<Item = &'a BindingMap> + 'a {
        let start = u32::try_from(offset).ok();
        self.hir_db()
            .graph()
            .declarations()
            .filter_map(move |declaration| {
                let start = start?;
                if declaration.span.source != source_id || !declaration.span.contains(start) {
                    return None;
                }
                match declaration.kind {
                    DeclarationKind::Function => self.hir_db().graph().bindings(declaration.id),
                    DeclarationKind::Trait => self.bindings_for_trait_method(declaration.id, start),
                    DeclarationKind::Impl => self.bindings_for_impl_method(declaration.id, start),
                    DeclarationKind::Const
                    | DeclarationKind::Struct
                    | DeclarationKind::Enum
                    | DeclarationKind::Global => None,
                }
            })
    }

    fn bindings_for_trait_method(
        &self,
        declaration: vela_hir::ids::HirDeclId,
        offset: u32,
    ) -> Option<&BindingMap> {
        self.hir_db()
            .graph()
            .trait_shape(declaration)?
            .methods
            .iter()
            .find_map(|method| {
                let body_span = method.default_body_span?;
                body_span
                    .contains(offset)
                    .then(|| {
                        method.default_body_node.and_then(|node| {
                            self.hir_db().graph().trait_default_method_bindings(node)
                        })
                    })
                    .flatten()
            })
    }

    fn bindings_for_impl_method(
        &self,
        declaration: vela_hir::ids::HirDeclId,
        offset: u32,
    ) -> Option<&BindingMap> {
        self.hir_db()
            .graph()
            .impl_metadata(declaration)?
            .methods
            .iter()
            .find_map(|method| {
                method
                    .span
                    .contains(offset)
                    .then(|| self.hir_db().graph().impl_method_bindings(method.node))
                    .flatten()
            })
    }

    fn script_method_signatures(&self, lookup: &MemberCallLookup<'_>) -> Vec<SignatureInformation> {
        let Some(receiver) = receiver_type_fact(
            lookup.text,
            lookup.source_id,
            lookup.bindings,
            lookup.facts,
            lookup.method_range,
        ) else {
            return Vec::new();
        };
        let owner_names = record_owner_names(&receiver);
        lookup
            .graph
            .declarations()
            .filter_map(|declaration| {
                if declaration.kind != DeclarationKind::Impl {
                    return None;
                }
                let metadata = lookup.graph.impl_metadata(declaration.id)?;
                if !matches!(metadata.kind, ImplMetadataKind::Inherent) {
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
                let method = metadata
                    .methods
                    .iter()
                    .find(|entry| entry.name == lookup.method)?;
                let owner = metadata.target_path.join("::");
                Some(method_signature_information(
                    lookup.graph,
                    self.schema_db().facts(),
                    &format!("{owner}.{}", method.name),
                    &method.signature,
                ))
            })
            .collect()
    }

    fn schema_method_signatures(&self, lookup: &MemberCallLookup<'_>) -> Vec<SignatureInformation> {
        let Some(receiver) = schema_receiver_type_fact(
            lookup.text,
            lookup.source_id,
            lookup.bindings,
            lookup.facts,
            self.schema_db().facts(),
            lookup.method_range,
        ) else {
            return Vec::new();
        };
        let Some((owner, fact)) =
            schema_method_fact_for_receiver(self.schema_db().facts(), &receiver, lookup.method)
        else {
            return Vec::new();
        };
        let TypeFact::Function { params, returns } = fact else {
            return Vec::new();
        };
        vec![SignatureInformation {
            label: signature_label(
                &format!("{}.{method}", owner, method = lookup.method),
                &schema_parameters(params),
                returns,
            ),
            parameters: schema_parameters(params),
        }]
    }

    fn stdlib_method_signatures(
        &self,
        lookup: &MemberCallLookup<'_>,
        args_prefix: &str,
    ) -> Vec<SignatureInformation> {
        let Some(receiver) = schema_receiver_type_fact(
            lookup.text,
            lookup.source_id,
            lookup.bindings,
            lookup.facts,
            self.schema_db().facts(),
            lookup.method_range,
        ) else {
            return Vec::new();
        };
        let lambda_param_count = first_lambda_param_count(args_prefix);
        let Some(fact) = stdlib_method_fact_with_lambda_arity(
            &receiver,
            lookup.method,
            None,
            lambda_param_count,
        ) else {
            return Vec::new();
        };
        vec![stdlib_method_signature_information(&fact)]
    }

    fn script_signatures(&self, callee: &str) -> Vec<SignatureInformation> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        graph
            .declarations()
            .filter(|declaration| {
                declaration.kind == DeclarationKind::Function
                    && (declaration.name == callee
                        || qualified_declaration_label(graph, declaration.id) == callee)
            })
            .filter_map(|declaration| {
                let fact = facts.declaration(declaration.id)?;
                let TypeFact::Function { params, returns } = fact else {
                    return None;
                };
                let signature = graph.function_signature(declaration.id)?;
                let parameters = signature
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        let type_fact = params.get(index).cloned().unwrap_or(TypeFact::Unknown);
                        let type_fact = if matches!(type_fact, TypeFact::Unknown) {
                            param
                                .type_hint
                                .as_ref()
                                .and_then(|hint| {
                                    schema_fact_for_hint(hint, self.schema_db().facts())
                                })
                                .unwrap_or(TypeFact::Unknown)
                        } else {
                            type_fact
                        };
                        SignatureParameter {
                            label: format!("{}: {}", param.name, type_fact.display_name()),
                            type_fact,
                        }
                    })
                    .collect::<Vec<_>>();
                Some(SignatureInformation {
                    label: signature_label(&declaration.name, &parameters, returns),
                    parameters,
                })
            })
            .collect()
    }

    fn script_variant_signatures(&self, callee: &str) -> Vec<SignatureInformation> {
        let graph = self.hir_db().graph();
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
                    if !variant_callee_matches(
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
                    let parameters = fields
                        .iter()
                        .map(|field| {
                            let fact = field.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                                signature_type_fact(graph, hint, self.schema_db().facts())
                            });
                            SignatureParameter {
                                label: format!("{}: {}", field.name, fact.display_name()),
                                type_fact: fact,
                            }
                        })
                        .collect::<Vec<_>>();
                    Some(SignatureInformation {
                        label: signature_label(
                            &format!("{owner}::{}", variant.name),
                            &parameters,
                            &TypeFact::enum_type(&owner, Some(&variant.name)),
                        ),
                        parameters,
                    })
                })
            })
            .collect()
    }

    fn schema_signatures(&self, callee: &str) -> Vec<SignatureInformation> {
        self.schema_db()
            .facts()
            .functions()
            .filter(|function| {
                function.name == callee
                    || function
                        .name
                        .rsplit("::")
                        .next()
                        .is_some_and(|name| name == callee)
            })
            .filter_map(|function| {
                let TypeFact::Function { params, returns } = function.fact else {
                    return None;
                };
                let parameters = params
                    .iter()
                    .enumerate()
                    .map(|(index, fact)| SignatureParameter {
                        label: format!("arg{index}: {}", fact.display_name()),
                        type_fact: fact.clone(),
                    })
                    .collect::<Vec<_>>();
                Some(SignatureInformation {
                    label: signature_label(&function.name, &parameters, &returns),
                    parameters,
                })
            })
            .collect()
    }
}

fn call_context_at(text: &str, position: Position) -> Option<CallContext> {
    let offset = LineIndex::new(text).offset(position);
    let open = active_call_open(text, offset)?;
    let (callee, callee_range) = callee_before_open(text, open)?;
    let args_prefix = text[open + 1..offset].to_owned();
    Some(CallContext {
        callee,
        callee_range,
        active_parameter: active_parameter_index(&args_prefix),
        args_prefix,
    })
}

fn active_call_open(text: &str, offset: usize) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in text[..offset].char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn callee_before_open(text: &str, open: usize) -> Option<(String, TextRange)> {
    let before = text[..open].trim_end();
    let end = before.len();
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_callee_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| (before[start..end].to_owned(), TextRange::new(start, end)))
}

fn active_parameter_index(args_text: &str) -> usize {
    let mut depth = 0usize;
    let mut active = 0usize;
    let mut lambda_params = false;
    for ch in args_text.chars() {
        match ch {
            '|' => lambda_params = !lambda_params,
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 && !lambda_params => active = active.saturating_add(1),
            _ => {}
        }
    }
    active
}

fn is_callee_continue(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}

fn signature_label(name: &str, parameters: &[SignatureParameter], returns: &TypeFact) -> String {
    let params = parameters
        .iter()
        .map(|param| param.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({params}) -> {}", returns.display_name())
}

fn method_signature_information(
    graph: &ModuleGraph,
    schema: &vela_analysis::registry::RegistryFacts,
    name: &str,
    signature: &vela_hir::type_hint::FunctionSignature,
) -> SignatureInformation {
    let parameters = signature
        .params
        .iter()
        .filter(|param| param.name != "self")
        .map(|param| {
            let type_fact = param.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                signature_type_fact(graph, hint, schema)
            });
            SignatureParameter {
                label: format!("{}: {}", param.name, type_fact.display_name()),
                type_fact,
            }
        })
        .collect::<Vec<_>>();
    let returns = signature
        .return_type
        .as_ref()
        .map_or(TypeFact::Unknown, |hint| {
            signature_type_fact(graph, hint, schema)
        });
    SignatureInformation {
        label: signature_label(name, &parameters, &returns),
        parameters,
    }
}

fn schema_parameters(params: &[TypeFact]) -> Vec<SignatureParameter> {
    params
        .iter()
        .enumerate()
        .map(|(index, fact)| SignatureParameter {
            label: format!("arg{index}: {}", fact.display_name()),
            type_fact: fact.clone(),
        })
        .collect()
}

fn stdlib_method_signature_information(fact: &StdlibMethodFact) -> SignatureInformation {
    let parameters = stdlib_method_parameters(fact);
    SignatureInformation {
        label: signature_label(
            &format!("{}.{}", fact.receiver.display_name(), fact.method),
            &parameters,
            &fact.returns,
        ),
        parameters,
    }
}

fn stdlib_function_signatures(callee: &str) -> Vec<SignatureInformation> {
    stdlib_function_completion_facts()
        .into_iter()
        .filter(|fact| {
            fact.name == callee
                || fact
                    .name
                    .rsplit("::")
                    .next()
                    .is_some_and(|name| name == callee)
        })
        .map(|fact| stdlib_function_signature_information(&fact))
        .collect()
}

fn stdlib_function_signature_information(fact: &StdlibFunctionFact) -> SignatureInformation {
    let parameters = schema_parameters(&fact.params);
    SignatureInformation {
        label: signature_label(fact.name, &parameters, &fact.returns),
        parameters,
    }
}

fn stdlib_method_parameters(fact: &StdlibMethodFact) -> Vec<SignatureParameter> {
    fact.params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let name = if is_lambda_parameter(param, fact.lambda.as_ref()) {
                "callback".to_owned()
            } else {
                format!("arg{index}")
            };
            SignatureParameter {
                label: format!("{name}: {}", param.display_name()),
                type_fact: param.clone(),
            }
        })
        .collect()
}

fn is_lambda_parameter(param: &TypeFact, lambda: Option<&LambdaFact>) -> bool {
    let Some(lambda) = lambda else {
        return false;
    };
    param == &TypeFact::function(lambda.params.clone(), lambda.returns.clone())
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

fn receiver_type_fact(
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    method_range: TextRange,
) -> Option<TypeFact> {
    let span = receiver_span(text, source_id, method_range)?;
    let resolution = bindings.resolution_at_span(span)?;
    type_fact_for_resolution(resolution, facts)
}

fn schema_receiver_type_fact(
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &vela_analysis::registry::RegistryFacts,
    method_range: TextRange,
) -> Option<TypeFact> {
    let span = receiver_span(text, source_id, method_range)?;
    let resolution = bindings.resolution_at_span(span)?;
    match resolution {
        BindingResolution::Local(local) => facts
            .local(*local)
            .cloned()
            .filter(|fact| !matches!(fact, TypeFact::Unknown))
            .or_else(|| {
                bindings
                    .local(*local)
                    .and_then(|binding| schema_fact_for_hint(binding.type_hint.as_ref()?, schema))
            }),
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn receiver_span(text: &str, source_id: SourceId, method_range: TextRange) -> Option<Span> {
    let receiver = member_receiver_range(text, method_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    Some(Span::new(source_id, start, end))
}

fn member_receiver_range(text: &str, member_start: usize) -> Option<TextRange> {
    let before_member = text.get(..member_start)?.trim_end();
    let before_dot = before_member.strip_suffix('.')?.trim_end();
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    facts: &AnalysisFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => facts
            .local(*local)
            .cloned()
            .filter(|fact| !matches!(fact, TypeFact::Unknown)),
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_method_fact_for_receiver<'a>(
    schema: &'a vela_analysis::registry::RegistryFacts,
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

fn variant_callee_matches(callee: &str, enum_name: &str, owner: &str, variant: &str) -> bool {
    callee == variant
        || callee == format!("{enum_name}::{variant}")
        || callee == format!("{owner}::{variant}")
}

fn signature_type_fact(
    graph: &ModuleGraph,
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> TypeFact {
    let fact = type_fact_from_hint(graph, hint);
    if matches!(fact, TypeFact::Unknown) {
        schema_fact_for_hint(hint, schema).unwrap_or(TypeFact::Unknown)
    } else {
        fact
    }
}

fn schema_fact_for_hint(
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    if !hint.args.is_empty() {
        return None;
    }
    let qualified = hint.path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
        .or_else(|| schema.trait_fact(&qualified))
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

#[cfg(test)]
mod tests {
    use vela_analysis::registry::RegistryFacts;

    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn signature_help_tracks_active_parameter() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn grant(player: Player, amount: i64) -> bool { return true }
            pub fn main(player: Player) { grant(player, 1) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(2).expect("main line should exist");
        let argument_offset = main_line
            .find("1)")
            .expect("second argument should exist in main call");
        let position = Position::new(2, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve script function");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "grant(player: Player, amount: i64) -> bool"
        );
        assert_eq!(help.signatures()[0].parameters()[1].label(), "amount: i64");
    }

    #[test]
    fn signature_help_resolves_script_method_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            struct Player { level: i64 }
            impl Player {
                fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
            }
            pub fn main(player: Player) { player.grant(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let main_line = text.lines().nth(5).expect("main line should exist");
        let argument_offset = main_line
            .find("2)")
            .expect("second argument should exist in method call");
        let position = Position::new(5, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve script method");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "Player.grant(amount: i64, bonus: i64) -> i64"
        );
        assert_eq!(help.signatures()[0].parameters()[0].label(), "amount: i64");
        assert_eq!(help.signatures()[0].parameters()[1].label(), "bonus: i64");
    }

    #[test]
    fn signature_help_resolves_schema_method_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn main(player: Player) { player.grant(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_method(
            "Player",
            "grant",
            TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::BOOL),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(1).expect("main line should exist");
        let argument_offset = main_line
            .find("2)")
            .expect("second argument should exist in method call");
        let position = Position::new(1, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve schema method");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "Player.grant(arg0: i64, arg1: i64) -> bool"
        );
        assert_eq!(help.signatures()[0].parameters()[1].label(), "arg1: i64");
    }

    #[test]
    fn signature_help_resolves_schema_trait_method_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn main(rewardable: Rewardable) { rewardable.preview(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_trait("Rewardable", TypeFact::trait_type("Rewardable"));
        schema.insert_trait_method(
            "Rewardable",
            "preview",
            TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::BOOL),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(1).expect("main line should exist");
        let argument_offset = main_line
            .find("2)")
            .expect("second argument should exist in trait method call");
        let position = Position::new(1, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve schema trait method");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "Rewardable.preview(arg0: i64, arg1: i64) -> bool"
        );
        assert_eq!(help.signatures()[0].parameters()[1].label(), "arg1: i64");
    }

    #[test]
    fn signature_help_resolves_stdlib_callback_method_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn main(scores: Array<i64>) {
                scores.filter(|score| score > 0)
            }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let filter_line = text.lines().nth(2).expect("filter line should exist");
        let position = Position::new(
            2,
            filter_line
                .find("score >")
                .expect("lambda body should contain score"),
        );
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve stdlib callback method");

        assert_eq!(help.active_parameter(), 0);
        assert_eq!(
            help.signatures()[0].label(),
            "Array(i64).filter(callback: Function(i64) -> bool) -> Array(i64)"
        );
        assert_eq!(
            help.signatures()[0].parameters()[0].label(),
            "callback: Function(i64) -> bool"
        );
    }

    #[test]
    fn signature_help_resolves_stdlib_function_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn main() { math::max(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let main_line = text.lines().nth(1).expect("main line should exist");
        let position = Position::new(1, main_line.find("2)").expect("second argument"));
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve stdlib function");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "math::max(arg0: i64 | f64, arg1: i64 | f64) -> i64 | f64"
        );
        assert_eq!(
            help.signatures()[0].parameters()[1].label(),
            "arg1: i64 | f64"
        );
    }
}
