use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    TextRange,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Definition {
    document_id: DocumentId,
    range: DiagnosticRange,
}

impl Definition {
    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DefinitionToken {
    range: TextRange,
    text: String,
    member_receiver: Option<TextRange>,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn definition(&self, document_id: &DocumentId, position: Position) -> Option<Definition> {
        let query = QueryContext::from_databases(self, document_id, position)?;
        let token = definition_token_at(&query)?;
        let source_id = query.source_id()?;
        let offset = u32::try_from(token.range.start).ok()?;
        let graph = self.hir_db().graph();

        if let Some(receiver) = token.member_receiver
            && let Some(receiver_fact) = query.type_fact_for_range(self, receiver)
            && let Some(definition) = self.schema_member_definition(&receiver_fact, &token)
        {
            return Some(definition);
        }

        if let Some(definition) = self.schema_variant_definition(query.text(), &token) {
            return Some(definition);
        }

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(offset) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if let Some(definition) = definition_from_resolution_at_token(bindings, &token, self) {
                return Some(definition);
            }
            if let Some(binding) = local_declaration_at_token(bindings, &token, self) {
                return self.definition_from_span(binding.span);
            }
        }

        self.schema_definition_for_token(&token).or_else(|| {
            graph
                .declarations()
                .find(|declaration| {
                    declaration.span.source == source_id && declaration.span.contains(offset)
                })
                .and_then(|declaration| self.definition_from_declaration(declaration))
        })
    }

    #[must_use]
    pub fn declaration(&self, document_id: &DocumentId, position: Position) -> Option<Definition> {
        self.definition(document_id, position)
    }

    #[must_use]
    pub fn type_definition(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<Definition> {
        self.definition(document_id, position)
    }

    fn definition_from_span(&self, span: Span) -> Option<Definition> {
        let source = self.source_record_for(span.source)?;
        let start = usize::try_from(span.start).ok()?;
        let end = usize::try_from(span.end).ok()?;
        let range = diagnostic_range(source.text(), TextRange::new(start, end));
        Some(Definition {
            document_id: source.document_id().clone(),
            range,
        })
    }

    fn definition_from_declaration(
        &self,
        declaration: &vela_hir::module_graph::Declaration,
    ) -> Option<Definition> {
        let source = self.source_record_for(declaration.span.source)?;
        let start = usize::try_from(declaration.span.start).ok()?;
        let end = usize::try_from(declaration.span.end).ok()?;
        let range = name_range_in_text(
            source.text(),
            TextRange::new(start, end),
            declaration.name.as_str(),
        )
        .unwrap_or(TextRange::new(start, end));
        Some(Definition {
            document_id: source.document_id().clone(),
            range: diagnostic_range(source.text(), range),
        })
    }

    fn source_record_for(&self, source_id: SourceId) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }

    fn schema_definition_for_token(&self, token: &DefinitionToken) -> Option<Definition> {
        let locations = self.schema_db().source_locations();
        let span = locations
            .type_span(&token.text)
            .or_else(|| locations.trait_span(&token.text))
            .or_else(|| locations.function_span(&token.text))?;
        self.definition_from_span(span)
    }

    fn schema_member_definition(
        &self,
        receiver_fact: &TypeFact,
        token: &DefinitionToken,
    ) -> Option<Definition> {
        let owner = fact_owner_name(receiver_fact)?;
        let locations = self.schema_db().source_locations();
        let span = locations
            .field_span(&owner, &token.text)
            .or_else(|| locations.method_span(&owner, &token.text))
            .or_else(|| locations.trait_method_span(&owner, &token.text))?;
        self.definition_from_span(span)
    }

    fn schema_variant_definition(&self, text: &str, token: &DefinitionToken) -> Option<Definition> {
        let path = path_ending_at(text, token.range)?;
        let (variant, owner_segments) = path.split_last()?;
        let owner = schema_variant_owner(self.schema_db().facts(), owner_segments, variant)?;
        let span = self
            .schema_db()
            .source_locations()
            .variant_span(&owner, variant)?;
        self.definition_from_span(span)
    }
}

fn definition_from_resolution_at_token(
    bindings: &BindingMap,
    token: &DefinitionToken,
    databases: &LanguageServiceDatabases,
) -> Option<Definition> {
    let graph = databases.hir_db().graph();
    let resolution = bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= token.range.start && token.range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)?
        .1;

    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            databases.definition_from_span(binding.span)
        }
        BindingResolution::Declaration(declaration) => {
            let declaration = graph.declaration(*declaration)?;
            databases.definition_from_declaration(declaration)
        }
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn local_declaration_at_token<'a>(
    bindings: &'a BindingMap,
    token: &DefinitionToken,
    databases: &LanguageServiceDatabases,
) -> Option<&'a LocalBinding> {
    bindings.locals().find(|binding| {
        let Ok(start) = usize::try_from(binding.span.start) else {
            return false;
        };
        let Ok(end) = usize::try_from(binding.span.end) else {
            return false;
        };
        let Some(source) = databases.source_record_for(binding.span.source) else {
            return false;
        };
        let Some(name_range) =
            name_range_in_text(source.text(), TextRange::new(start, end), &binding.name)
        else {
            return false;
        };
        name_range.start <= token.range.start && token.range.end <= name_range.end
    })
}

fn definition_token_at(query: &QueryContext<'_>) -> Option<DefinitionToken> {
    let text = query.text();
    let range = query.identifier_range()?;
    Some(DefinitionToken {
        text: text[range.start..range.end].to_owned(),
        range,
        member_receiver: query.member_receiver_range(),
    })
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    let relative = slice.find(name)?;
    let start = range.start + relative;
    Some(TextRange::new(start, start + name.len()))
}

fn fact_owner_name(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Host { name }
        | TypeFact::Record { name }
        | TypeFact::Enum { name, .. }
        | TypeFact::Trait { name } => Some(name.clone()),
        _ => None,
    }
}

fn path_ending_at(text: &str, range: TextRange) -> Option<Vec<String>> {
    let mut path = vec![text.get(range.start..range.end)?.to_owned()];
    let mut cursor = range.start;
    loop {
        let before_segment = text.get(..cursor)?.trim_end();
        let Some(before_separator) = before_segment.strip_suffix("::").map(str::trim_end) else {
            break;
        };
        let end = before_separator.len();
        let start = before_separator
            .char_indices()
            .rev()
            .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
            .unwrap_or(0);
        if start == end {
            break;
        }
        path.push(text.get(start..end)?.to_owned());
        cursor = start;
    }
    (path.len() > 1).then(|| {
        path.reverse();
        path
    })
}

fn schema_variant_owner(
    schema: &RegistryFacts,
    owner_segments: &[String],
    variant: &str,
) -> Option<String> {
    if owner_segments.is_empty() {
        return None;
    }
    let owner = owner_segments.join("::");
    if schema.variant_fact(&owner, variant).is_some() {
        return Some(owner);
    }
    if owner.contains("::") {
        return None;
    }
    let mut matches = schema.variants().filter_map(|candidate| {
        (candidate.name == variant
            && candidate
                .owner
                .rsplit("::")
                .next()
                .is_some_and(|short| short == owner))
        .then_some(candidate.owner)
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn definition_follows_local_binding() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let definition = databases
            .definition(
                &document,
                Position::new(0, text.rfind("amount").expect("amount use")),
            )
            .expect("definition should resolve parameter binding");

        assert_eq!(definition.document_id(), &document);
        assert_eq!(definition.range().start().line, 0);
        assert_eq!(
            definition.range().start().character,
            text.find("amount")
                .expect("parameter declaration should exist")
        );
    }

    #[test]
    fn declaration_follows_local_binding() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let declaration = databases
            .declaration(
                &document,
                Position::new(0, text.rfind("amount").expect("amount use")),
            )
            .expect("declaration should resolve parameter binding");

        assert_eq!(declaration.document_id(), &document);
        assert_eq!(declaration.range().start().line, 0);
        assert_eq!(
            declaration.range().start().character,
            text.find("amount")
                .expect("parameter declaration should exist")
        );
    }

    #[test]
    fn definition_follows_function_call_after_qualified_stdlib_call() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
fn add_mixed(value) {
    math::abs(value);
    return value + 1i8;
}

fn main() {
    return add_mixed(1);
}
"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let definition = databases
            .definition(
                &document,
                Position::new(
                    7,
                    text.lines()
                        .nth(7)
                        .expect("call line")
                        .find("add_mixed")
                        .expect("call should exist"),
                ),
            )
            .expect("definition should resolve function call");

        assert_eq!(definition.document_id(), &document);
        assert_eq!(definition.range().start().line, 1);
        assert_eq!(definition.range().start().character, 3);
    }

    #[test]
    fn type_definition_follows_schema_source_span() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let main_text = "pub fn main(player: Player) { return 1 }";
        let schema_text = "pub fn host_player_schema() { return 1 }";
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find("host_player_schema")
            .expect("schema marker should exist");
        let target_end = target_start + "host_player_schema".len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" },
                        "sourceSpan": {
                            "source": schema_record.source_id().get(),
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .type_definition(
                &main,
                Position::new(0, main_text.find("Player").expect("type hint should exist")),
            )
            .expect("type definition should resolve schema source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().line, 0);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    #[test]
    fn definition_follows_imported_module_declaration() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/helper.vela");
        let main_text = "use game::helper::grant\npub fn main() { return grant() }";
        let helper_text = "pub fn grant() -> i64 { return 1 }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(helper.clone(), helper_text),
        ]);
        let call_line = main_text.lines().nth(1).expect("call line should exist");

        let definition = databases
            .definition(
                &main,
                Position::new(1, call_line.find("grant").expect("grant call")),
            )
            .expect("definition should resolve imported function");

        assert_eq!(definition.document_id(), &helper);
        assert_eq!(definition.range().start().line, 0);
        assert_eq!(
            definition.range().start().character,
            helper_text.find("grant").expect("helper function name")
        );
    }

    #[test]
    fn definition_follows_schema_source_span() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let main_text = "pub fn main(player: Player) { return 1 }";
        let schema_text = "pub fn host_player_schema() { return 1 }";
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find("host_player_schema")
            .expect("schema marker should exist");
        let target_end = target_start + "host_player_schema".len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" },
                        "sourceSpan": {
                            "source": schema_record.source_id().get(),
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .definition(
                &main,
                Position::new(0, main_text.find("Player").expect("type hint should exist")),
            )
            .expect("definition should resolve schema source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().line, 0);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    #[test]
    fn definition_follows_schema_field_source_span() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let main_text = "pub fn main(player: Player) { return player.level }";
        let schema_text = "pub fn level_marker() { return 1 }";
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find("level_marker")
            .expect("schema marker should exist");
        let target_end = target_start + "level_marker".len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "fields": [
                    {
                        "owner": "Player",
                        "name": "level",
                        "fact": { "kind": "primitive", "name": "i64" },
                        "sourceSpan": {
                            "source": schema_record.source_id().get(),
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .definition(
                &main,
                Position::new(0, main_text.find("level").expect("field use should exist")),
            )
            .expect("definition should resolve schema field source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    #[test]
    fn definition_follows_schema_method_source_span() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let main_text = "pub fn main(player: Player) { return player.grant(1) }";
        let schema_text = "pub fn grant_marker() { return true }";
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find("grant_marker")
            .expect("schema marker should exist");
        let target_end = target_start + "grant_marker".len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "Player",
                        "fact": { "kind": "host", "name": "Player" }
                    }
                ],
                "methods": [
                    {
                        "owner": "Player",
                        "name": "grant",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": schema_record.source_id().get(),
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .definition(
                &main,
                Position::new(0, main_text.find("grant").expect("method use should exist")),
            )
            .expect("definition should resolve schema method source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    #[test]
    fn definition_follows_schema_trait_method_source_span() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let main_text = "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }";
        let schema_text = "pub fn preview_marker() { return true }";
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find("preview_marker")
            .expect("schema marker should exist");
        let target_end = target_start + "preview_marker".len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "traits": [
                    {
                        "name": "Rewardable",
                        "fact": { "kind": "trait", "name": "Rewardable" }
                    }
                ],
                "traitMethods": [
                    {
                        "owner": "Rewardable",
                        "name": "preview",
                        "fact": {
                            "kind": "function",
                            "params": [{ "kind": "primitive", "name": "i64" }],
                            "returns": { "kind": "primitive", "name": "bool" }
                        },
                        "sourceSpan": {
                            "source": schema_record.source_id().get(),
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .definition(
                &main,
                Position::new(
                    0,
                    main_text
                        .find("preview")
                        .expect("trait method use should exist"),
                ),
            )
            .expect("definition should resolve schema trait method source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    #[test]
    fn definition_follows_schema_variant_source_span() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let main_text = "pub fn main() { return QuestState::Active }";
        let schema_text = "pub fn active_marker() { return 1 }";
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find("active_marker")
            .expect("schema marker should exist");
        let target_end = target_start + "active_marker".len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": {
                "types": [
                    {
                        "name": "QuestState",
                        "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                    }
                ],
                "variants": [
                    {
                        "owner": "QuestState",
                        "name": "Active",
                        "fact": {
                            "kind": "enum",
                            "name": "QuestState",
                            "variant": "Active"
                        },
                        "sourceSpan": {
                            "source": schema_record.source_id().get(),
                            "start": target_start,
                            "end": target_end
                        }
                    }
                ]
            }
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .definition(
                &main,
                Position::new(
                    0,
                    main_text.find("Active").expect("variant use should exist"),
                ),
            )
            .expect("definition should resolve schema variant source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    #[test]
    fn type_definition_follows_schema_field_source_span() {
        assert_schema_member_type_definition(
            "pub fn main(player: Player) { return player.level }",
            "level",
            "pub fn level_marker() { return 1 }",
            "level_marker",
            |source, start, end| {
                serde_json::json!({
                    "types": [
                        {
                            "name": "Player",
                            "fact": { "kind": "host", "name": "Player" }
                        }
                    ],
                    "fields": [
                        {
                            "owner": "Player",
                            "name": "level",
                            "fact": { "kind": "primitive", "name": "i64" },
                            "sourceSpan": {
                                "source": source,
                                "start": start,
                                "end": end
                            }
                        }
                    ]
                })
            },
        );
    }

    #[test]
    fn type_definition_follows_schema_method_source_span() {
        assert_schema_member_type_definition(
            "pub fn main(player: Player) { return player.grant(1) }",
            "grant",
            "pub fn grant_marker() { return true }",
            "grant_marker",
            |source, start, end| {
                serde_json::json!({
                    "types": [
                        {
                            "name": "Player",
                            "fact": { "kind": "host", "name": "Player" }
                        }
                    ],
                    "methods": [
                        {
                            "owner": "Player",
                            "name": "grant",
                            "fact": {
                                "kind": "function",
                                "params": [{ "kind": "primitive", "name": "i64" }],
                                "returns": { "kind": "primitive", "name": "bool" }
                            },
                            "sourceSpan": {
                                "source": source,
                                "start": start,
                                "end": end
                            }
                        }
                    ]
                })
            },
        );
    }

    #[test]
    fn type_definition_follows_schema_trait_method_source_span() {
        assert_schema_member_type_definition(
            "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }",
            "preview",
            "pub fn preview_marker() { return true }",
            "preview_marker",
            |source, start, end| {
                serde_json::json!({
                    "traits": [
                        {
                            "name": "Rewardable",
                            "fact": { "kind": "trait", "name": "Rewardable" }
                        }
                    ],
                    "traitMethods": [
                        {
                            "owner": "Rewardable",
                            "name": "preview",
                            "fact": {
                                "kind": "function",
                                "params": [{ "kind": "primitive", "name": "i64" }],
                                "returns": { "kind": "primitive", "name": "bool" }
                            },
                            "sourceSpan": {
                                "source": source,
                                "start": start,
                                "end": end
                            }
                        }
                    ]
                })
            },
        );
    }

    #[test]
    fn type_definition_follows_schema_variant_source_span() {
        let main_text = "pub fn main() { return QuestState::Active }";
        assert_schema_member_type_definition(
            main_text,
            "Active",
            "pub fn active_marker() { return 1 }",
            "active_marker",
            |source, start, end| {
                serde_json::json!({
                    "types": [
                        {
                            "name": "QuestState",
                            "fact": { "kind": "enum", "name": "QuestState", "variant": null }
                        }
                    ],
                    "variants": [
                        {
                            "owner": "QuestState",
                            "name": "Active",
                            "fact": {
                                "kind": "enum",
                                "name": "QuestState",
                                "variant": "Active"
                            },
                            "sourceSpan": {
                                "source": source,
                                "start": start,
                                "end": end
                            }
                        }
                    ]
                })
            },
        );
    }

    fn assert_schema_member_type_definition<F>(
        main_text: &str,
        usage_needle: &str,
        schema_text: &str,
        schema_marker: &str,
        facts: F,
    ) where
        F: FnOnce(u32, usize, usize) -> serde_json::Value,
    {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let schema_source = DocumentId::from("/workspace/scripts/schema_defs.vela");
        let mut databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(schema_source.clone(), schema_text),
        ]);
        let schema_record = databases
            .source_db()
            .records()
            .get(&schema_source)
            .expect("schema source should be indexed");
        let target_start = schema_text
            .find(schema_marker)
            .expect("schema marker should exist");
        let target_end = target_start + schema_marker.len();
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": facts(schema_record.source_id().get(), target_start, target_end)
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases
            .type_definition(
                &main,
                Position::new(0, main_text.find(usage_needle).expect("usage should exist")),
            )
            .expect("type definition should resolve schema source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
