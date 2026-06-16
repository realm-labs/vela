use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
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
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn definition(&self, document_id: &DocumentId, position: Position) -> Option<Definition> {
        let source = self.source_db().records().get(document_id)?;
        let token = definition_token_at(source.text(), position)?;
        let source_id = source.source_id();
        let offset = u32::try_from(token.range.start).ok()?;
        let graph = self.hir_db().graph();

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
                .and_then(|declaration| self.definition_from_span(declaration.span))
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
            databases.definition_from_span(declaration.span)
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

fn definition_token_at(text: &str, position: Position) -> Option<DefinitionToken> {
    let offset = LineIndex::new(text).offset(position);
    let range = identifier_range_at(text, offset)?;
    Some(DefinitionToken {
        text: text[range.start..range.end].to_owned(),
        range,
    })
}

fn identifier_range_at(text: &str, offset: usize) -> Option<TextRange> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| TextRange::new(start, end))
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
        assert_eq!(definition.range().start().character, 0);
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

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
