use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::stdlib::{LambdaFact, StdlibMethodFact, stdlib_method_fact_with_lambda_arity};
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{HirTypeHint, ImplMetadataKind};

use crate::query_context::{
    CallableFacts, CallableOrigin, callable_facts, type_fact_for_source_range,
};
use crate::{DocumentId, LanguageServiceDatabases, Position, QueryContext, TextRange};

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
    member_receiver: Option<TextRange>,
    args_prefix: String,
    active_parameter: usize,
}

struct MemberCallLookup<'a> {
    graph: &'a ModuleGraph,
    receiver: &'a TypeFact,
    method: &'a str,
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
        let context = call_context_from_query(&query)?;
        let signatures = self.signature_candidates_for_context(Some(&query), source_id, &context);
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
        let callables = callable_facts(self, callee);
        self.signature_candidates_from_callables(&callables)
    }

    pub(crate) fn signature_candidates_for_member_call(
        &self,
        source_id: SourceId,
        callee: String,
        member_receiver: TextRange,
        args_prefix: String,
    ) -> Vec<SignatureInformation> {
        let context = CallContext {
            callee,
            member_receiver: Some(member_receiver),
            args_prefix,
            active_parameter: 0,
        };
        self.signature_candidates_for_context(None, source_id, &context)
    }

    fn signature_candidates_for_context(
        &self,
        query: Option<&QueryContext<'_>>,
        source_id: SourceId,
        context: &CallContext,
    ) -> Vec<SignatureInformation> {
        if let Some(signatures) = self.member_signatures(source_id, context)
            && !signatures.is_empty()
        {
            return signatures;
        }
        if let Some(query) = query {
            let callables = query.callable_facts(self, &context.callee);
            self.signature_candidates_from_callables(&callables)
        } else {
            self.signature_candidates(&context.callee)
        }
    }

    fn signature_candidates_from_callables(
        &self,
        callables: &[CallableFacts],
    ) -> Vec<SignatureInformation> {
        let mut signatures = callable_signatures_by_origin(callables, CallableOrigin::Source);
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::SourceVariant,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::Schema,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::Stdlib,
        ));
        signatures
    }

    fn member_signatures(
        &self,
        source_id: SourceId,
        context: &CallContext,
    ) -> Option<Vec<SignatureInformation>> {
        let (_receiver, method) = context.callee.rsplit_once('.')?;
        if method.is_empty() {
            return None;
        }
        let receiver_range = context.member_receiver?;
        let receiver = type_fact_for_source_range(self, source_id, receiver_range)?;
        let graph = self.hir_db().graph();
        let lookup = MemberCallLookup {
            graph,
            receiver: &receiver,
            method,
        };
        let mut signatures = self.script_method_signatures(&lookup);
        signatures.extend(self.schema_method_signatures(&lookup));
        signatures.extend(self.stdlib_method_signatures(&lookup, &context.args_prefix));
        (!signatures.is_empty()).then_some(signatures)
    }

    fn script_method_signatures(&self, lookup: &MemberCallLookup<'_>) -> Vec<SignatureInformation> {
        let owner_names = record_owner_names(lookup.receiver);
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
        let Some((owner, fact)) = schema_method_fact_for_receiver(
            self.schema_db().facts(),
            lookup.receiver,
            lookup.method,
        ) else {
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
        let lambda_param_count = first_lambda_param_count(args_prefix);
        let Some(fact) = stdlib_method_fact_with_lambda_arity(
            lookup.receiver,
            lookup.method,
            None,
            lambda_param_count,
        ) else {
            return Vec::new();
        };
        vec![stdlib_method_signature_information(&fact)]
    }
}

fn call_context_from_query(query: &QueryContext<'_>) -> Option<CallContext> {
    let call = query.call_argument_facts()?;
    Some(CallContext {
        callee: call.callee().to_owned(),
        member_receiver: call.member_receiver(),
        active_parameter: call.active_parameter(),
        args_prefix: call.args_prefix().to_owned(),
    })
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

fn callable_signature_information(callable: &CallableFacts) -> SignatureInformation {
    let parameters = callable
        .params()
        .iter()
        .map(|param| {
            let type_fact = param.type_fact().clone();
            SignatureParameter {
                label: format!("{}: {}", param.name(), type_fact.display_name()),
                type_fact,
            }
        })
        .collect::<Vec<_>>();
    SignatureInformation {
        label: signature_label(callable.name(), &parameters, callable.returns()),
        parameters,
    }
}

fn callable_signatures_by_origin(
    callables: &[CallableFacts],
    origin: CallableOrigin,
) -> Vec<SignatureInformation> {
    callables
        .iter()
        .filter(|callable| callable.origin() == origin)
        .map(callable_signature_information)
        .collect()
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

#[cfg(test)]
mod tests {
    use vela_analysis::registry::RegistryFacts;

    use super::*;
    use crate::{
        LineIndex, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
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
    fn signature_help_uses_shared_context_for_incomplete_calls() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn grant(player: Player, amount: i64) -> bool { return true }
            pub fn main(player: Player) { grant(
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        databases.set_schema_facts(schema);
        databases.update(&project);

        let line_index = LineIndex::new(text);
        let position = line_index.position(text.find("grant(").expect("call") + "grant(".len());
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve incomplete call");

        assert_eq!(help.active_parameter(), 0);
        assert_eq!(
            help.signatures()[0].label(),
            "grant(player: Player, amount: i64) -> bool"
        );
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
    fn signature_help_resolves_source_enum_variant_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            enum QuestState { Finished(quest_id: String) }
            pub fn main() { Finished("quest-1") }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let main_line = text.lines().nth(2).expect("main line should exist");
        let position = Position::new(2, main_line.find("\"quest").expect("variant argument"));
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve enum variant");

        assert_eq!(help.active_parameter(), 0);
        assert_eq!(
            help.signatures()[0].label(),
            "game::main::QuestState::Finished(quest_id: String) -> game::main::QuestState::Finished"
        );
        assert_eq!(
            help.signatures()[0].parameters()[0].label(),
            "quest_id: String"
        );
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
