use vela_analysis::{
    registry::{RegistryFunctionFact, RegistryMemberFact},
    type_fact::TypeFact,
};
use vela_common::Span;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature};

use crate::{
    DiagnosticRange, DisplayParts, DocumentId, LanguageServiceDatabases, LineIndex, SourceRecord,
    SymbolRef, TextRange,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DocumentSymbol {
    name: String,
    detail: Option<String>,
    kind: DocumentSymbolKind,
    range: DiagnosticRange,
    selection_range: DiagnosticRange,
    children: Vec<DocumentSymbol>,
}

impl DocumentSymbol {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    #[must_use]
    pub const fn kind(&self) -> DocumentSymbolKind {
        self.kind
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub const fn selection_range(&self) -> DiagnosticRange {
        self.selection_range
    }

    #[must_use]
    pub fn children(&self) -> &[DocumentSymbol] {
        &self.children
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceSymbol {
    name: String,
    detail: Option<String>,
    kind: DocumentSymbolKind,
    container_name: Option<String>,
    location: WorkspaceSymbolLocation,
    symbol: SymbolRef,
}

impl WorkspaceSymbol {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    #[must_use]
    pub const fn kind(&self) -> DocumentSymbolKind {
        self.kind
    }

    #[must_use]
    pub fn container_name(&self) -> Option<&str> {
        self.container_name.as_deref()
    }

    #[must_use]
    pub const fn location(&self) -> &WorkspaceSymbolLocation {
        &self.location
    }

    #[must_use]
    pub const fn symbol(&self) -> &SymbolRef {
        &self.symbol
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorkspaceSymbolLocation {
    Source {
        document_id: DocumentId,
        range: DiagnosticRange,
    },
    Schema,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DocumentSymbolKind {
    Class,
    Constant,
    Enum,
    EnumMember,
    Field,
    File,
    Function,
    Interface,
    Method,
    Module,
    Object,
    Struct,
    TypeParameter,
    Variable,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn document_symbols(&self, document_id: &DocumentId) -> Vec<DocumentSymbol> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let mut symbols = graph
            .declarations()
            .filter(|declaration| declaration.span.source == source.source_id())
            .filter_map(|declaration| symbol_from_declaration(graph, declaration, source))
            .collect::<Vec<_>>();
        symbols.sort_by_key(|symbol| {
            let start = symbol.range.start();
            (start.line, start.character)
        });
        symbols
    }

    #[must_use]
    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let query = query.trim();
        let mut symbols = self
            .file_workspace_symbols(query)
            .into_iter()
            .chain(self.module_workspace_symbols(query))
            .chain(self.script_workspace_symbols(query))
            .chain(self.schema_workspace_symbols(query))
            .collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.kind_name().cmp(right.kind_name()))
        });
        symbols
    }

    fn file_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.source_db()
            .records()
            .iter()
            .filter_map(|(document_id, source)| {
                let name = file_symbol_name(document_id)?;
                if !symbol_matches(query, &name) {
                    return None;
                }
                Some(WorkspaceSymbol {
                    name,
                    detail: self
                        .project_db()
                        .module_by_document()
                        .get(document_id)
                        .map(|module_path| module_path.join())
                        .filter(|module| !module.is_empty()),
                    kind: DocumentSymbolKind::File,
                    container_name: None,
                    location: WorkspaceSymbolLocation::Source {
                        document_id: document_id.clone(),
                        range: diagnostic_range(
                            source.text(),
                            TextRange::new(0, source.text().len()),
                        ),
                    },
                    symbol: SymbolRef::Source(document_id.as_str().to_owned()),
                })
            })
            .collect()
    }

    fn module_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.project_db()
            .module_by_document()
            .iter()
            .filter_map(|(document_id, module_path)| {
                let name = module_path.join();
                if name.is_empty() || !symbol_matches(query, &name) {
                    return None;
                }
                let source = self.source_db().records().get(document_id)?;
                Some(WorkspaceSymbol {
                    symbol: SymbolRef::Source(name.clone()),
                    name,
                    detail: None,
                    kind: DocumentSymbolKind::Module,
                    container_name: parent_module_name(module_path.segments()),
                    location: WorkspaceSymbolLocation::Source {
                        document_id: document_id.clone(),
                        range: diagnostic_range(
                            source.text(),
                            TextRange::new(0, source.text().len()),
                        ),
                    },
                })
            })
            .collect()
    }

    fn script_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let graph = self.hir_db().graph();
        graph
            .declarations()
            .filter_map(|declaration| {
                let module_path = graph.module_path(declaration.module)?;
                let module = module_path.join();
                let name = if module.is_empty() {
                    declaration.name.clone()
                } else {
                    DisplayParts::qualified(&module, &declaration.name).render()
                };
                symbol_matches(query, &name).then(|| {
                    let source = self.symbol_source_record_for(declaration.span.source)?;
                    let range = diagnostic_range(source.text(), span_range(declaration.span)?);
                    Some(WorkspaceSymbol {
                        symbol: SymbolRef::Source(name.clone()),
                        name,
                        detail: detail_for_declaration(graph, declaration),
                        kind: kind_for_declaration(declaration.kind),
                        container_name: (!module.is_empty()).then_some(module),
                        location: WorkspaceSymbolLocation::Source {
                            document_id: source.document_id().clone(),
                            range,
                        },
                    })
                })?
            })
            .collect()
    }

    fn schema_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let facts = self.schema_db().facts();
        let mut symbols = Vec::new();
        symbols.extend(facts.types().filter_map(|(name, fact)| {
            schema_symbol(
                query,
                name,
                Some(fact.display_name()),
                schema_type_symbol_kind(fact),
                None,
                SymbolRef::Schema(name.to_owned()),
            )
        }));
        symbols.extend(facts.traits().filter_map(|(name, fact)| {
            schema_symbol(
                query,
                name,
                Some(fact.display_name()),
                DocumentSymbolKind::Interface,
                None,
                SymbolRef::Schema(name.to_owned()),
            )
        }));
        symbols.extend(facts.functions().filter_map(|function| {
            schema_function_symbol(query, function, DocumentSymbolKind::Function)
        }));
        symbols.extend(
            facts.fields().filter_map(|member| {
                schema_member_symbol(query, member, DocumentSymbolKind::Field)
            }),
        );
        symbols.extend(facts.variants().filter_map(|member| {
            schema_member_symbol(query, member, DocumentSymbolKind::EnumMember)
        }));
        symbols.extend(
            facts.methods().filter_map(|member| {
                schema_member_symbol(query, member, DocumentSymbolKind::Method)
            }),
        );
        symbols.extend(
            facts.trait_methods().filter_map(|member| {
                schema_member_symbol(query, member, DocumentSymbolKind::Method)
            }),
        );
        symbols
    }

    fn symbol_source_record_for(&self, source_id: vela_common::SourceId) -> Option<&SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }
}

impl WorkspaceSymbol {
    fn kind_name(&self) -> &'static str {
        match self.kind {
            DocumentSymbolKind::Constant => "constant",
            DocumentSymbolKind::Class => "class",
            DocumentSymbolKind::Enum => "enum",
            DocumentSymbolKind::EnumMember => "enum_member",
            DocumentSymbolKind::Field => "field",
            DocumentSymbolKind::File => "file",
            DocumentSymbolKind::Function => "function",
            DocumentSymbolKind::Interface => "interface",
            DocumentSymbolKind::Method => "method",
            DocumentSymbolKind::Module => "module",
            DocumentSymbolKind::Object => "object",
            DocumentSymbolKind::Struct => "struct",
            DocumentSymbolKind::TypeParameter => "type_parameter",
            DocumentSymbolKind::Variable => "variable",
        }
    }
}

fn parent_module_name(segments: &[String]) -> Option<String> {
    (segments.len() > 1).then(|| segments[..segments.len() - 1].join("::"))
}

fn file_symbol_name(document_id: &DocumentId) -> Option<String> {
    document_id
        .as_str()
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .map(str::to_owned)
}

fn symbol_from_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
    source: &SourceRecord,
) -> Option<DocumentSymbol> {
    let kind = kind_for_declaration(declaration.kind);
    let children = children_for_declaration(graph, declaration, source);
    symbol_from_span(
        source,
        declaration.span,
        declaration.name.clone(),
        detail_for_declaration(graph, declaration),
        kind,
        children,
    )
}

fn kind_for_declaration(kind: DeclarationKind) -> DocumentSymbolKind {
    match kind {
        DeclarationKind::Const => DocumentSymbolKind::Constant,
        DeclarationKind::Global => DocumentSymbolKind::Variable,
        DeclarationKind::Function => DocumentSymbolKind::Function,
        DeclarationKind::Struct => DocumentSymbolKind::Struct,
        DeclarationKind::Enum => DocumentSymbolKind::Enum,
        DeclarationKind::Trait => DocumentSymbolKind::Interface,
        DeclarationKind::Impl => DocumentSymbolKind::Object,
    }
}

fn children_for_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
    source: &SourceRecord,
) -> Vec<DocumentSymbol> {
    match declaration.kind {
        DeclarationKind::Struct => graph
            .struct_shape(declaration.id)
            .into_iter()
            .flat_map(|shape| &shape.fields)
            .filter_map(|field| {
                symbol_from_span(
                    source,
                    field.span,
                    field.name.clone(),
                    field.type_hint.as_ref().map(|hint| hint.display()),
                    DocumentSymbolKind::Field,
                    Vec::new(),
                )
            })
            .collect(),
        DeclarationKind::Enum => graph
            .enum_shape(declaration.id)
            .into_iter()
            .flat_map(|shape| &shape.variants)
            .filter_map(|variant| {
                let children = match &variant.fields {
                    EnumVariantFieldsHint::Unit => Vec::new(),
                    EnumVariantFieldsHint::Tuple(params) => params
                        .iter()
                        .filter_map(|param| {
                            symbol_from_span(
                                source,
                                param.span,
                                param.name.clone(),
                                param.type_hint.as_ref().map(|hint| hint.display()),
                                DocumentSymbolKind::Field,
                                Vec::new(),
                            )
                        })
                        .collect(),
                    EnumVariantFieldsHint::Record(fields) => fields
                        .iter()
                        .filter_map(|field| {
                            symbol_from_span(
                                source,
                                field.span,
                                field.name.clone(),
                                field.type_hint.as_ref().map(|hint| hint.display()),
                                DocumentSymbolKind::Field,
                                Vec::new(),
                            )
                        })
                        .collect(),
                };
                symbol_from_span(
                    source,
                    variant.span,
                    variant.name.clone(),
                    None,
                    DocumentSymbolKind::EnumMember,
                    children,
                )
            })
            .collect(),
        DeclarationKind::Trait => graph
            .trait_shape(declaration.id)
            .into_iter()
            .flat_map(|shape| &shape.methods)
            .filter_map(|method| {
                symbol_from_span(
                    source,
                    method.span,
                    method.name.clone(),
                    Some(signature_detail(&method.signature)),
                    DocumentSymbolKind::Method,
                    Vec::new(),
                )
            })
            .collect(),
        DeclarationKind::Impl => graph
            .impl_metadata(declaration.id)
            .into_iter()
            .flat_map(|metadata| &metadata.methods)
            .filter_map(|method| {
                symbol_from_span(
                    source,
                    method.span,
                    method.name.clone(),
                    Some(signature_detail(&method.signature)),
                    DocumentSymbolKind::Method,
                    Vec::new(),
                )
            })
            .collect(),
        DeclarationKind::Const | DeclarationKind::Global | DeclarationKind::Function => Vec::new(),
    }
}

fn detail_for_declaration(graph: &ModuleGraph, declaration: &Declaration) -> Option<String> {
    match declaration.kind {
        DeclarationKind::Const => graph
            .const_metadata(declaration.id)
            .and_then(|metadata| metadata.type_hint.as_ref().map(|hint| hint.display())),
        DeclarationKind::Global => graph
            .global_metadata(declaration.id)
            .map(|metadata| metadata.type_hint.display()),
        DeclarationKind::Function => graph
            .function_signature(declaration.id)
            .map(signature_detail),
        DeclarationKind::Struct
        | DeclarationKind::Enum
        | DeclarationKind::Trait
        | DeclarationKind::Impl => None,
    }
}

fn signature_detail(signature: &FunctionSignature) -> String {
    let params = signature.params.iter().map(|param| {
        param.type_hint.as_ref().map_or_else(
            || DisplayParts::plain(param.name.as_str()),
            |hint| DisplayParts::parameter(&param.name, &hint.display()),
        )
    });
    let return_type = signature.return_type.as_ref().map(|hint| hint.display());
    DisplayParts::signature(params, return_type.as_deref()).render()
}

fn symbol_from_span(
    source: &SourceRecord,
    span: Span,
    name: String,
    detail: Option<String>,
    kind: DocumentSymbolKind,
    children: Vec<DocumentSymbol>,
) -> Option<DocumentSymbol> {
    if span.source != source.source_id() {
        return None;
    }
    let range = diagnostic_range(source.text(), span_range(span)?);
    let selection_range = name_range_in_span(source.text(), span, &name)
        .map_or(range, |range| diagnostic_range(source.text(), range));
    Some(DocumentSymbol {
        name,
        detail,
        kind,
        range,
        selection_range,
        children,
    })
}

fn span_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_span(text: &str, span: Span, name: &str) -> Option<TextRange> {
    let span_range = span_range(span)?;
    let slice = text.get(span_range.start..span_range.end)?;
    let offset = slice.find(name)?;
    Some(TextRange::new(
        span_range.start + offset,
        span_range.start + offset + name.len(),
    ))
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn schema_function_symbol(
    query: &str,
    function: RegistryFunctionFact,
    kind: DocumentSymbolKind,
) -> Option<WorkspaceSymbol> {
    let name = function.name;
    let detail = function.fact.display_name();
    schema_symbol(
        query,
        &name,
        Some(detail),
        kind,
        None,
        SymbolRef::Schema(name.clone()),
    )
}

fn schema_member_symbol(
    query: &str,
    member: RegistryMemberFact,
    kind: DocumentSymbolKind,
) -> Option<WorkspaceSymbol> {
    let name = DisplayParts::qualified(&member.owner, &member.name).render();
    let symbol = schema_member_symbol_ref(&member, kind);
    schema_symbol(
        query,
        &name,
        Some(member.fact.display_name()),
        kind,
        Some(member.owner),
        symbol,
    )
}

fn schema_symbol(
    query: &str,
    name: &str,
    detail: Option<String>,
    kind: DocumentSymbolKind,
    container_name: Option<String>,
    symbol: SymbolRef,
) -> Option<WorkspaceSymbol> {
    symbol_matches(query, name).then(|| WorkspaceSymbol {
        name: name.to_owned(),
        detail,
        kind,
        container_name,
        location: WorkspaceSymbolLocation::Schema,
        symbol,
    })
}

fn schema_member_symbol_ref(member: &RegistryMemberFact, kind: DocumentSymbolKind) -> SymbolRef {
    let symbol = if kind == DocumentSymbolKind::EnumMember {
        format!("{}::{}", member.owner, member.name)
    } else {
        format!("{}.{}", member.owner, member.name)
    };
    SymbolRef::Schema(symbol)
}

fn schema_type_symbol_kind(fact: &TypeFact) -> DocumentSymbolKind {
    match fact {
        TypeFact::Host { .. } => DocumentSymbolKind::Class,
        TypeFact::Record { .. } => DocumentSymbolKind::Struct,
        TypeFact::Enum { variant: None, .. } => DocumentSymbolKind::Enum,
        TypeFact::Enum {
            variant: Some(_), ..
        } => DocumentSymbolKind::EnumMember,
        TypeFact::Trait { .. } => DocumentSymbolKind::Interface,
        TypeFact::Module { .. } => DocumentSymbolKind::Module,
        TypeFact::Function { .. } => DocumentSymbolKind::Function,
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
        | TypeFact::Union(_) => DocumentSymbolKind::Struct,
    }
}

fn symbol_matches(query: &str, name: &str) -> bool {
    query.is_empty()
        || name
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn document_symbols_include_nested_type_members() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
const BASE: i64 = 1
global player: Player
pub struct Player {
    level: i64
    name: String
}
pub enum Reward {
    None
    Coins(amount: i64)
    Item { id: String }
}
pub trait Damageable {
    fn damage(amount: i64) -> bool
}
impl Player {
    fn heal(amount: i64) -> i64 { return amount }
}
pub fn main(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let symbols = databases.document_symbols(&document);

        assert_eq!(
            symbol_names(&symbols),
            [
                "BASE",
                "player",
                "Player",
                "Reward",
                "Damageable",
                "impl Player",
                "main"
            ]
        );
        let player = symbol(&symbols, "Player");
        assert_eq!(player.kind(), DocumentSymbolKind::Struct);
        assert_eq!(symbol_names(player.children()), ["level", "name"]);
        assert_eq!(
            player.children()[0].selection_range().start().line,
            3,
            "struct field selection should point at the field line"
        );

        let reward = symbol(&symbols, "Reward");
        assert_eq!(reward.kind(), DocumentSymbolKind::Enum);
        assert_eq!(symbol_names(reward.children()), ["None", "Coins", "Item"]);
        assert_eq!(symbol_names(reward.children()[1].children()), ["amount"]);
        assert_eq!(symbol_names(reward.children()[2].children()), ["id"]);

        let damageable = symbol(&symbols, "Damageable");
        assert_eq!(damageable.kind(), DocumentSymbolKind::Interface);
        assert_eq!(symbol_names(damageable.children()), ["damage"]);

        let impl_player = symbol(&symbols, "impl Player");
        assert_eq!(impl_player.kind(), DocumentSymbolKind::Object);
        assert_eq!(symbol_names(impl_player.children()), ["heal"]);

        let main = symbol(&symbols, "main");
        assert_eq!(main.detail(), Some("(amount: i64) -> i64"));
        assert_eq!(main.kind(), DocumentSymbolKind::Function);
    }

    #[test]
    fn workspace_symbols_include_module_qualified_names() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main, "pub fn main() -> i64 { return grant() }"),
            SourceFileSnapshot::new(
                helper.clone(),
                "pub struct Reward { amount: i64 }\npub fn grant() -> Reward { return Reward { amount: 1 } }",
            ),
        ]);

        let symbols = databases.workspace_symbols("reward");

        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name() == "game::reward::Reward"
                    && symbol.kind() == DocumentSymbolKind::Struct
                    && symbol.symbol()
                        == &SymbolRef::Source("game::reward::Reward".to_owned())
                    && matches!(
                        symbol.location(),
                        WorkspaceSymbolLocation::Source { document_id, .. } if document_id == &helper
                    )),
            "{symbols:?}"
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name() == "game::reward::grant"
                    && symbol.container_name() == Some("game::reward")
                    && symbol.symbol() == &SymbolRef::Source("game::reward::grant".to_owned())),
            "{symbols:?}"
        );
    }

    #[test]
    fn workspace_symbols_include_module_symbols() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main, "pub fn main() -> i64 { return 1 }"),
            SourceFileSnapshot::new(reward.clone(), "pub fn grant() -> i64 { return 1 }"),
        ]);

        let symbols = databases.workspace_symbols("game::reward");

        assert!(
            symbols.iter().any(|symbol| symbol.name() == "game::reward"
                && symbol.kind() == DocumentSymbolKind::Module
                && symbol.symbol() == &SymbolRef::Source("game::reward".to_owned())
                && matches!(
                    symbol.location(),
                    WorkspaceSymbolLocation::Source { document_id, .. } if document_id == &reward
                )),
            "{symbols:?}"
        );
    }

    #[test]
    fn workspace_symbols_include_file_symbols() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main, "pub fn main() -> i64 { return 1 }"),
            SourceFileSnapshot::new(reward.clone(), "pub fn grant() -> i64 { return 1 }"),
        ]);

        let symbols = databases.workspace_symbols("reward.vela");

        assert!(
            symbols.iter().any(|symbol| symbol.name() == "reward.vela"
                && symbol.kind() == DocumentSymbolKind::File
                && symbol.detail() == Some("game::reward")
                && symbol.symbol() == &SymbolRef::Source(reward.as_str().to_owned())
                && matches!(
                    symbol.location(),
                    WorkspaceSymbolLocation::Source { document_id, .. } if document_id == &reward
                )),
            "{symbols:?}"
        );
    }

    #[test]
    fn workspace_symbols_include_schema_items() {
        let mut databases = databases_for(Vec::new());
        let mut facts = vela_analysis::registry::RegistryFacts::default();
        facts.insert_type("Player", vela_analysis::type_fact::TypeFact::host("Player"));
        facts.insert_type(
            "Inventory",
            vela_analysis::type_fact::TypeFact::record("Inventory"),
        );
        facts.insert_type(
            "QuestState",
            vela_analysis::type_fact::TypeFact::enum_type("QuestState", None::<String>),
        );
        facts.insert_trait(
            "Rewardable",
            vela_analysis::type_fact::TypeFact::trait_type("Rewardable"),
        );
        facts.insert_field("Player", "level", vela_analysis::type_fact::TypeFact::I64);
        facts.insert_method(
            "Player",
            "grant_exp",
            vela_analysis::type_fact::TypeFact::function(
                vec![vela_analysis::type_fact::TypeFact::I64],
                vela_analysis::type_fact::TypeFact::BOOL,
            ),
        );
        facts.insert_function(
            "game::reward::grant",
            vela_analysis::type_fact::TypeFact::function(
                Vec::new(),
                vela_analysis::type_fact::TypeFact::BOOL,
            ),
        );
        databases.set_schema_facts(facts);

        let symbols = databases.workspace_symbols("Player");

        assert!(
            symbols.iter().any(|symbol| symbol.name() == "Player"
                && symbol.kind() == DocumentSymbolKind::Class
                && symbol.symbol() == &SymbolRef::Schema("Player".to_owned())
                && matches!(symbol.location(), WorkspaceSymbolLocation::Schema)),
            "{symbols:?}"
        );
        assert!(
            symbols.iter().any(|symbol| symbol.name() == "Player::level"
                && symbol.kind() == DocumentSymbolKind::Field
                && symbol.detail() == Some("i64")
                && symbol.symbol() == &SymbolRef::Schema("Player.level".to_owned())),
            "{symbols:?}"
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name() == "Player::grant_exp"
                    && symbol.kind() == DocumentSymbolKind::Method
                    && symbol.container_name() == Some("Player")
                    && symbol.symbol() == &SymbolRef::Schema("Player.grant_exp".to_owned())),
            "{symbols:?}"
        );

        let symbols = databases.workspace_symbols("");
        assert!(
            symbols.iter().any(|symbol| symbol.name() == "Inventory"
                && symbol.kind() == DocumentSymbolKind::Struct),
            "{symbols:?}"
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name() == "QuestState"
                    && symbol.kind() == DocumentSymbolKind::Enum),
            "{symbols:?}"
        );
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }

    fn symbol<'a>(symbols: &'a [DocumentSymbol], name: &str) -> &'a DocumentSymbol {
        symbols
            .iter()
            .find(|symbol| symbol.name() == name)
            .unwrap_or_else(|| panic!("symbol `{name}` should exist"))
    }

    fn symbol_names<const N: usize>(symbols: &[DocumentSymbol]) -> [&str; N] {
        let names = symbols.iter().map(DocumentSymbol::name).collect::<Vec<_>>();
        names.try_into().unwrap_or_else(|names: Vec<&str>| {
            panic!("expected {N} symbols, got {}: {names:?}", names.len())
        })
    }
}
