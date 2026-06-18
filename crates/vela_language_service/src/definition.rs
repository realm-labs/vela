use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    SymbolRef, TextRange,
    symbol_ref::{qualified_source_declaration_name, source_symbol_for_declaration},
    symbol_target::SymbolTarget,
};

mod source_members;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Definition {
    document_id: DocumentId,
    range: DiagnosticRange,
    symbol: Option<SymbolRef>,
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

    #[must_use]
    pub fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn definition(&self, document_id: &DocumentId, position: Position) -> Option<Definition> {
        let query = QueryContext::from_databases(self, document_id, position)?;
        let target = SymbolTarget::from_query(self, &query)?;
        let source_id = query.source_id()?;
        let offset = u32::try_from(target.range().start).ok()?;
        let graph = self.hir_db().graph();

        if target.is_schema_symbol()
            && let Some(definition) = target.schema_member_span(self).and_then(|span| {
                self.definition_from_span_with_symbol(span, target.symbol().cloned())
            })
        {
            return Some(definition);
        }

        if let Some(definition) = target
            .schema_variant_target(self, &query)
            .and_then(|(span, symbol)| self.definition_from_span_with_symbol(span, Some(symbol)))
        {
            return Some(definition);
        }

        if let Some(definition) = source_members::source_member_definition_for_target(self, &target)
        {
            return Some(definition);
        }

        if query.member_receiver_range().is_some() {
            return None;
        }

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(offset) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if let Some(definition) = definition_from_resolution_at_target(bindings, &target, self)
            {
                return Some(definition);
            }
            if let Some(binding) = local_declaration_at_target(bindings, &target, self) {
                return self.definition_from_span_with_symbol(
                    binding.span,
                    Some(
                        target
                            .symbol()
                            .cloned()
                            .unwrap_or_else(|| self.definition_local_symbol_for_binding(binding)),
                    ),
                );
            }
        }

        if target.is_schema_symbol() {
            return self.schema_definition_for_target(&target);
        }

        graph
            .declarations()
            .find(|declaration| {
                declaration.span.source == source_id && declaration.span.contains(offset)
            })
            .and_then(|declaration| self.definition_from_declaration(declaration))
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
        let query = QueryContext::from_databases(self, document_id, position)?;
        let target = SymbolTarget::from_query(self, &query)?;

        if let Some(fact) = self.member_type_fact_for_target(&target) {
            return self.type_definition_for_fact(&fact);
        }

        if let Some(fact) = query.type_fact_for_range(self, target.range())
            && let Some(definition) = self.type_definition_for_fact(&fact)
        {
            return Some(definition);
        }

        if target.is_schema_symbol() {
            return self.schema_type_definition_for_name(target.text());
        }

        None
    }

    fn definition_from_span_with_symbol(
        &self,
        span: Span,
        symbol: Option<SymbolRef>,
    ) -> Option<Definition> {
        let source = self.source_record_for(span.source)?;
        let start = usize::try_from(span.start).ok()?;
        let end = usize::try_from(span.end).ok()?;
        let range = diagnostic_range(source.text(), TextRange::new(start, end));
        Some(Definition {
            document_id: source.document_id().clone(),
            range,
            symbol,
        })
    }

    fn definition_from_declaration(&self, declaration: &Declaration) -> Option<Definition> {
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
            symbol: Some(source_symbol_for_declaration(
                self.hir_db().graph(),
                declaration,
            )),
        })
    }

    fn source_record_for(&self, source_id: SourceId) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }

    fn schema_definition_for_target(&self, target: &SymbolTarget) -> Option<Definition> {
        if !target.is_schema_symbol() {
            return None;
        }
        target
            .schema_symbol_span(self)
            .and_then(|span| self.definition_from_span_with_symbol(span, target.symbol().cloned()))
    }

    fn member_type_fact_for_target(&self, target: &SymbolTarget) -> Option<TypeFact> {
        source_members::source_field_type_fact_for_target(self, target)
            .or_else(|| self.schema_field_type_fact_for_target(target))
    }

    fn schema_field_type_fact_for_target(&self, target: &SymbolTarget) -> Option<TypeFact> {
        let owner = target.member_receiver_fact().and_then(fact_owner_name)?;
        self.schema_db()
            .facts()
            .field_fact(&owner, target.text())
            .cloned()
    }

    fn type_definition_for_fact(&self, fact: &TypeFact) -> Option<Definition> {
        match fact {
            TypeFact::Record { name } => self
                .source_type_definition_for_name(name, DeclarationKind::Struct)
                .or_else(|| self.schema_type_definition_for_name(name)),
            TypeFact::Enum { name, .. } => self
                .source_type_definition_for_name(name, DeclarationKind::Enum)
                .or_else(|| self.schema_type_definition_for_name(name)),
            TypeFact::Host { name } => self.schema_type_definition_for_name(name),
            TypeFact::Trait { name } => self
                .source_type_definition_for_name(name, DeclarationKind::Trait)
                .or_else(|| self.schema_trait_definition_for_name(name)),
            TypeFact::Union(facts) => facts
                .iter()
                .find_map(|fact| self.type_definition_for_fact(fact)),
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
            | TypeFact::Module { .. } => None,
        }
    }

    fn source_type_definition_for_name(
        &self,
        name: &str,
        kind: DeclarationKind,
    ) -> Option<Definition> {
        let declaration = source_declaration_for_fact_name(self.hir_db().graph(), name, kind)?;
        self.definition_from_declaration(declaration)
    }

    fn schema_type_definition_for_name(&self, name: &str) -> Option<Definition> {
        self.schema_db()
            .source_locations()
            .type_span(name)
            .or_else(|| {
                short_name(name)
                    .and_then(|short| self.schema_db().source_locations().type_span(short))
            })
            .and_then(|span| {
                self.definition_from_span_with_symbol(span, Some(SymbolRef::Schema(name.into())))
            })
    }

    fn schema_trait_definition_for_name(&self, name: &str) -> Option<Definition> {
        self.schema_db()
            .source_locations()
            .trait_span(name)
            .or_else(|| {
                short_name(name)
                    .and_then(|short| self.schema_db().source_locations().trait_span(short))
            })
            .and_then(|span| {
                self.definition_from_span_with_symbol(span, Some(SymbolRef::Schema(name.into())))
            })
    }

    fn definition_local_symbol_for_binding(&self, binding: &LocalBinding) -> SymbolRef {
        let Some(source) = self.source_record_for(binding.span.source) else {
            return SymbolRef::local(binding.name.clone());
        };
        SymbolRef::local_from_span(
            binding.name.clone(),
            source.document_id().clone(),
            source.text(),
            binding.span,
        )
    }
}

fn definition_from_resolution_at_target(
    bindings: &BindingMap,
    target: &SymbolTarget,
    databases: &LanguageServiceDatabases,
) -> Option<Definition> {
    let graph = databases.hir_db().graph();
    let resolution = bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= target.range().start && target.range().end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)?
        .1;

    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            let symbol = target
                .symbol()
                .cloned()
                .unwrap_or_else(|| databases.definition_local_symbol_for_binding(binding));
            databases.definition_from_span_with_symbol(binding.span, Some(symbol))
        }
        BindingResolution::Declaration(declaration) => {
            let declaration = graph.declaration(*declaration)?;
            let mut definition = databases.definition_from_declaration(declaration)?;
            let declaration_symbol = source_symbol_for_declaration(graph, declaration);
            if target.symbol() == Some(&declaration_symbol) {
                definition.symbol = Some(declaration_symbol);
            }
            Some(definition)
        }
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn local_declaration_at_target<'a>(
    bindings: &'a BindingMap,
    target: &SymbolTarget,
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
        name_range.start <= target.range().start && target.range().end <= name_range.end
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

fn source_declaration_for_fact_name<'a>(
    graph: &'a ModuleGraph,
    name: &str,
    kind: DeclarationKind,
) -> Option<&'a Declaration> {
    graph
        .declarations()
        .find(|declaration| {
            declaration.kind == kind
                && qualified_source_declaration_name(graph, declaration) == name
        })
        .or_else(|| {
            let short = short_name(name).unwrap_or(name);
            let mut matches = graph
                .declarations()
                .filter(|declaration| declaration.kind == kind && declaration.name == short);
            let declaration = matches.next()?;
            matches.next().is_none().then_some(declaration)
        })
}

fn short_name(name: &str) -> Option<&str> {
    name.rsplit("::").next().filter(|short| *short != name)
}

#[cfg(test)]
mod tests {
    use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};

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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::local_at(
                "amount",
                document.clone(),
                TextRange::new(12, 18)
            ))
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
        assert_eq!(
            declaration.symbol(),
            Some(&SymbolRef::local_at(
                "amount",
                document.clone(),
                TextRange::new(12, 18)
            ))
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Source("game::main::add_mixed".into()))
        );
    }

    #[test]
    fn definition_follows_source_struct_field_member_access() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Cell {
    value: i64,
}

fn assign_cell(cell: Cell, value) {
    cell.value = value;
    return cell.value;
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let field_line = text.lines().nth(5).expect("field write line");

        let definition = databases
            .definition(
                &document,
                Position::new(5, field_line.find("value").expect("field use")),
            )
            .expect("definition should resolve source field");

        assert_eq!(definition.document_id(), &document);
        assert_eq!(definition.range().start().line, 1);
        assert_eq!(
            definition.range().start().character,
            text.lines()
                .nth(1)
                .expect("field declaration line")
                .find("value")
                .expect("field declaration")
        );
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Source("game::main::Cell.value".into()))
        );
    }

    #[test]
    fn definition_does_not_fallback_to_enclosing_function_for_unknown_member() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Cell {
    value: i64,
}

fn assign_cell(cell: Cell) {
    return cell.missing;
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let use_line = text.lines().nth(5).expect("member use line");

        let definition = databases.definition(
            &document,
            Position::new(5, use_line.find("missing").expect("unknown field use")),
        );

        assert!(definition.is_none());
    }

    #[test]
    fn type_definition_follows_local_source_type() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Player {
    level: i64,
}

fn main(player: Player) {
    return player;
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let use_line = text.lines().nth(5).expect("player use line");

        let definition = databases
            .type_definition(
                &document,
                Position::new(5, use_line.find("player").expect("player use")),
            )
            .expect("type definition should resolve source struct");

        assert_eq!(definition.document_id(), &document);
        assert_eq!(definition.range().start().line, 0);
        assert_eq!(definition.range().start().character, 7);
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Source("game::main::Player".into()))
        );
    }

    #[test]
    fn type_definition_follows_source_field_type() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Inventory {
    slots: i64,
}

struct Player {
    inventory: Inventory,
}

fn main(player: Player) {
    return player.inventory;
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let use_line = text.lines().nth(9).expect("field use line");

        let definition = databases
            .type_definition(
                &document,
                Position::new(9, use_line.find("inventory").expect("field use")),
            )
            .expect("type definition should resolve source field type");

        assert_eq!(definition.document_id(), &document);
        assert_eq!(definition.range().start().line, 0);
        assert_eq!(definition.range().start().character, 7);
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Source("game::main::Inventory".into()))
        );
    }

    #[test]
    fn type_definition_returns_none_for_source_primitive_field() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"struct Cell {
    value: i64,
}

fn main(cell: Cell) {
    return cell.value;
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let use_line = text.lines().nth(5).expect("field use line");

        let definition = databases.type_definition(
            &document,
            Position::new(5, use_line.find("value").expect("field use")),
        );

        assert!(definition.is_none());
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("Player".into()))
        );
    }

    #[test]
    fn schema_type_without_source_span_does_not_fabricate_definition() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let main_text = "pub fn main(player: Player) { return 1 }";
        let mut databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), main_text)]);
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        databases.set_schema_facts(schema);

        let definition = databases.type_definition(
            &main,
            Position::new(
                0,
                main_text
                    .find("Player")
                    .expect("schema type hint should exist"),
            ),
        );

        assert!(definition.is_none());
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Source("game::helper::grant".into()))
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("Player".into()))
        );
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("Player.level".into()))
        );
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("Player.grant".into()))
        );
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("Rewardable.preview".into()))
        );
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
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("QuestState::Active".into()))
        );
    }

    #[test]
    fn definition_follows_qualified_schema_variant_when_name_is_not_unique() {
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
                    },
                    {
                        "name": "OtherState",
                        "fact": { "kind": "enum", "name": "OtherState", "variant": null }
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
                    },
                    {
                        "owner": "OtherState",
                        "name": "Active",
                        "fact": {
                            "kind": "enum",
                            "name": "OtherState",
                            "variant": "Active"
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
            .expect("definition should resolve qualified schema variant source span");

        assert_eq!(definition.document_id(), &schema_source);
        assert_eq!(definition.range().start().character, target_start);
        assert_eq!(definition.range().end().character, target_end);
        assert_eq!(
            definition.symbol(),
            Some(&SymbolRef::Schema("QuestState::Active".into()))
        );
    }

    #[test]
    fn type_definition_follows_schema_field_type_source_span() {
        assert_schema_member_type_definition(
            "pub fn main(player: Player) { return player.inventory }",
            "inventory",
            "pub fn inventory_type_marker() { return 1 }",
            "inventory_type_marker",
            |source, start, end| {
                serde_json::json!({
                    "types": [
                        {
                            "name": "Player",
                            "fact": { "kind": "host", "name": "Player" }
                        },
                        {
                            "name": "Inventory",
                            "fact": { "kind": "host", "name": "Inventory" },
                            "sourceSpan": {
                                "source": source,
                                "start": start,
                                "end": end
                            }
                        }
                    ],
                    "fields": [
                        {
                            "owner": "Player",
                            "name": "inventory",
                            "fact": { "kind": "host", "name": "Inventory" }
                        }
                    ]
                })
            },
        );
    }

    #[test]
    fn type_definition_returns_none_for_schema_primitive_field() {
        assert_schema_member_type_definition_none(
            "pub fn main(player: Player) { return player.level }",
            "level",
            "pub fn level_marker() { return 1 }",
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
    fn type_definition_returns_none_for_schema_method() {
        assert_schema_member_type_definition_none(
            "pub fn main(player: Player) { return player.grant(1) }",
            "grant",
            "pub fn grant_marker() { return true }",
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
    fn type_definition_returns_none_for_schema_trait_method() {
        assert_schema_member_type_definition_none(
            "pub fn main(rewardable: Rewardable) { return rewardable.preview(1) }",
            "preview",
            "pub fn preview_marker() { return true }",
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
    fn type_definition_returns_none_for_schema_variant_without_owner_type_span() {
        let main_text = "pub fn main() { return QuestState::Active }";
        assert_schema_member_type_definition_none(
            main_text,
            "Active",
            "pub fn active_marker() { return 1 }",
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

    fn assert_schema_member_type_definition_none<F>(
        main_text: &str,
        usage_needle: &str,
        schema_text: &str,
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
        let artifact = serde_json::json!({
            "formatVersion": 1,
            "facts": facts(
                schema_record.source_id().get(),
                0,
                schema_text.len(),
            )
        })
        .to_string();
        databases.load_schema_artifact_json("/workspace/target/vela/schema.json", &artifact);

        let definition = databases.type_definition(
            &main,
            Position::new(0, main_text.find(usage_needle).expect("usage should exist")),
        );

        assert!(definition.is_none());
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
