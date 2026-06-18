use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;

use crate::callable_context::{
    CallableFacts, CallableOrigin, callable_facts, member_callable_facts,
};
use crate::{
    DisplayParts, DocumentId, LanguageServiceDatabases, Position, QueryContext, TextRange,
};

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
    name: String,
    label: String,
    label_parts: DisplayParts,
    type_fact: TypeFact,
}

impl SignatureParameter {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn label_parts(&self) -> &DisplayParts {
        &self.label_parts
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
            CallableOrigin::SourceMethod,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::SourceVariant,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::SchemaVariant,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::Schema,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::SchemaMethod,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::Stdlib,
        ));
        signatures.extend(callable_signatures_by_origin(
            callables,
            CallableOrigin::StdlibMethod,
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
        let callables = member_callable_facts(
            self,
            source_id,
            receiver_range,
            method,
            &context.args_prefix,
        );
        let signatures = self.signature_candidates_from_callables(&callables);
        (!signatures.is_empty()).then_some(signatures)
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
    let returns = returns.display_name();
    DisplayParts::callable_signature(
        name,
        parameters.iter().map(|param| param.label_parts.clone()),
        Some(returns.as_str()),
    )
    .render()
}

fn callable_signature_information(callable: &CallableFacts) -> SignatureInformation {
    let parameters = callable
        .params()
        .iter()
        .map(|param| {
            let type_fact = param.type_fact().clone();
            let type_name = type_fact.display_name();
            let mut label_parts = DisplayParts::parameter(param.name(), &type_name);
            if param.defaulted() {
                label_parts.extend(DisplayParts::plain(" (defaulted)"));
            }
            SignatureParameter {
                name: param.name().to_owned(),
                label: label_parts.render(),
                label_parts,
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

#[cfg(test)]
mod tests {
    use vela_analysis::registry::RegistryFacts;

    use super::*;
    use crate::{
        DisplayPartKind, LineIndex, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
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
        assert_eq!(
            help.signatures()[0].parameters()[0].label_parts().parts()[0].kind(),
            DisplayPartKind::Parameter
        );
        assert_eq!(help.signatures()[0].parameters()[1].name(), "amount");
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
    fn signature_help_resolves_imported_function_with_defaulted_parameter() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let rewards = DocumentId::from("/workspace/scripts/game/rewards.vela");
        let main_text = "\
use game::rewards::reward_bonus
pub fn main(amount: i64) -> i64 {
    return reward_bonus(amount, 2)
}";
        let rewards_text = "\
pub fn reward_bonus(amount: i64, scale: i64 = 1) -> i64 {
    return amount * scale
}";
        let files = vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(rewards, rewards_text),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let call_line = main_text.lines().nth(2).expect("call line should exist");
        let position = Position::new(2, call_line.find("2)").expect("second argument"));
        let help = databases
            .signature_help(&main, position)
            .expect("signature help should resolve imported function");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "reward_bonus(amount: i64, scale: i64 (defaulted)) -> i64"
        );
        assert_eq!(
            help.signatures()[0].parameters()[1].label(),
            "scale: i64 (defaulted)"
        );
    }

    #[test]
    fn signature_help_returns_none_for_unknown_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { missing(1, 2) }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let position = LineIndex::new(text).position(text.find("2)").expect("second argument"));
        assert!(
            databases.signature_help(&document, position).is_none(),
            "unknown calls must not produce speculative signature help"
        );
    }

    #[test]
    fn signature_help_returns_none_for_dynamic_receiver_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player) { player.grant(1, 2) }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let position = LineIndex::new(text).position(text.find("2)").expect("second argument"));
        assert!(
            databases.signature_help(&document, position).is_none(),
            "dynamic receiver calls must not invent method signature facts"
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
    fn signature_help_resolves_source_trait_receiver_method_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            trait Rewardable {
                fn preview(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
            }
            pub fn main(rewardable: Rewardable) { rewardable.preview(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let main_line = text.lines().nth(4).expect("main line should exist");
        let argument_offset = main_line
            .find("2)")
            .expect("second argument should exist in trait method call");
        let position = Position::new(4, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve source trait receiver method");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "game::main::Rewardable.preview(amount: i64, bonus: i64) -> i64"
        );
        assert_eq!(help.signatures()[0].parameters()[0].name(), "amount");
        assert_eq!(help.signatures()[0].parameters()[1].name(), "bonus");
    }

    #[test]
    fn signature_help_resolves_source_trait_impl_method_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            trait Rewardable { fn grant(self, amount: i64, bonus: i64) -> i64; }
            struct Player { level: i64 }
            impl Rewardable for Player {
                fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
            }
            pub fn main(player: Player) { player.grant(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let main_line = text.lines().nth(6).expect("main line should exist");
        let argument_offset = main_line
            .find("2)")
            .expect("second argument should exist in trait impl method call");
        let position = Position::new(6, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve source trait impl method");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "Rewardable for Player.grant(amount: i64, bonus: i64) -> i64"
        );
        assert_eq!(help.signatures()[0].parameters()[0].name(), "amount");
        assert_eq!(help.signatures()[0].parameters()[1].name(), "bonus");
    }

    #[test]
    fn signature_help_resolves_source_trait_default_method_on_record_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            trait Rewardable {
                fn grant(self, amount: i64, bonus: i64) -> i64 { return amount + bonus }
            }
            struct Player { level: i64 }
            impl Rewardable for Player {}
            pub fn main(player: Player) { player.grant(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let main_line = text.lines().nth(6).expect("main line should exist");
        let argument_offset = main_line
            .find("2)")
            .expect("second argument should exist in trait default method call");
        let position = Position::new(6, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve source trait default method");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "game::main::Rewardable.grant(amount: i64, bonus: i64) -> i64"
        );
        assert_eq!(help.signatures()[0].parameters()[0].name(), "amount");
        assert_eq!(help.signatures()[0].parameters()[1].name(), "bonus");
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
    fn signature_help_resolves_schema_enum_variant_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn main() { QuestState::Active("quest-1", 3) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type(
            "QuestState",
            TypeFact::enum_type("QuestState", None::<String>),
        );
        schema.insert_variant(
            "QuestState",
            "Active",
            TypeFact::enum_type("QuestState", Some("Active")),
        );
        schema.insert_field("QuestState::Active", "0", TypeFact::STRING);
        schema.insert_field("QuestState::Active", "1", TypeFact::I64);
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(1).expect("main line should exist");
        let position = Position::new(1, main_line.find(", 3").expect("second argument") + 2);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve schema enum variant");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "QuestState::Active(arg0: String, arg1: i64) -> QuestState::Active"
        );
        assert_eq!(help.signatures()[0].parameters()[0].label(), "arg0: String");
        assert_eq!(help.signatures()[0].parameters()[1].label(), "arg1: i64");
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
    fn signature_help_resolves_schema_method_on_schema_function_return() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn main() { current_player().grant(1, 2) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_function(
            "current_player",
            TypeFact::function(Vec::new(), TypeFact::host("Player")),
        );
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
            .expect("signature help should resolve schema method on returned receiver");

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
