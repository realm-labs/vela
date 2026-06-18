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
    name_parts: DisplayParts,
    detail: Option<String>,
    detail_parts: Option<DisplayParts>,
    kind: DocumentSymbolKind,
    range: DiagnosticRange,
    selection_range: DiagnosticRange,
    children: Vec<DocumentSymbol>,
    symbol: SymbolRef,
}

impl DocumentSymbol {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn name_parts(&self) -> &DisplayParts {
        &self.name_parts
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    #[must_use]
    pub fn detail_parts(&self) -> Option<&DisplayParts> {
        self.detail_parts.as_ref()
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

    #[must_use]
    pub const fn symbol(&self) -> &SymbolRef {
        &self.symbol
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceSymbol {
    name: String,
    name_parts: DisplayParts,
    detail: Option<String>,
    detail_parts: Option<DisplayParts>,
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
    pub const fn name_parts(&self) -> &DisplayParts {
        &self.name_parts
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    #[must_use]
    pub fn detail_parts(&self) -> Option<&DisplayParts> {
        self.detail_parts.as_ref()
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
                let name_parts = DisplayParts::symbol(name);
                let detail_parts = self
                    .project_db()
                    .module_by_document()
                    .get(document_id)
                    .map(|module_path| DisplayParts::symbol(module_path.join()))
                    .filter(|module| !module.render().is_empty());
                Some(WorkspaceSymbol {
                    name: name_parts.render(),
                    name_parts,
                    detail: detail_parts.as_ref().map(DisplayParts::render),
                    detail_parts,
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
                let name_parts = DisplayParts::symbol(name);
                Some(WorkspaceSymbol {
                    symbol: SymbolRef::Source(name_parts.render()),
                    name: name_parts.render(),
                    name_parts,
                    detail: None,
                    detail_parts: None,
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
                    DisplayParts::symbol(&declaration.name)
                } else {
                    DisplayParts::qualified(&module, &declaration.name)
                };
                let rendered_name = name.render();
                symbol_matches(query, &rendered_name).then(|| {
                    let source = self.symbol_source_record_for(declaration.span.source)?;
                    let range = diagnostic_range(source.text(), span_range(declaration.span)?);
                    let detail_parts = detail_parts_for_declaration(graph, declaration);
                    Some(WorkspaceSymbol {
                        symbol: SymbolRef::Source(rendered_name.clone()),
                        name: rendered_name,
                        name_parts: name,
                        detail: detail_parts.as_ref().map(DisplayParts::render),
                        detail_parts,
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
            let detail = fact.display_name();
            schema_symbol(
                query,
                DisplayParts::symbol(name),
                Some(DisplayParts::type_name(detail)),
                schema_type_symbol_kind(fact),
                None,
                SymbolRef::Schema(name.to_owned()),
            )
        }));
        symbols.extend(facts.traits().filter_map(|(name, fact)| {
            let detail = fact.display_name();
            schema_symbol(
                query,
                DisplayParts::symbol(name),
                Some(DisplayParts::type_name(detail)),
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
    let symbol_name = source_declaration_symbol_name(graph, declaration)?;
    let children = children_for_declaration(graph, declaration, source, &symbol_name);
    symbol_from_span(
        source,
        declaration.span,
        DisplayParts::symbol(&declaration.name),
        detail_parts_for_declaration(graph, declaration),
        kind,
        children,
        SymbolRef::Source(symbol_name),
    )
}

fn source_declaration_symbol_name(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> Option<String> {
    let module_path = graph.module_path(declaration.module)?;
    if module_path.segments().is_empty() {
        Some(declaration.name.clone())
    } else {
        Some(DisplayParts::qualified(&module_path.join(), &declaration.name).render())
    }
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
    parent_symbol: &str,
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
                    DisplayParts::symbol(&field.name),
                    field
                        .type_hint
                        .as_ref()
                        .map(|hint| DisplayParts::type_name(hint.display())),
                    DocumentSymbolKind::Field,
                    Vec::new(),
                    SymbolRef::Source(format!("{parent_symbol}.{}", field.name)),
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
                            let variant_symbol = format!("{parent_symbol}::{}", variant.name);
                            symbol_from_span(
                                source,
                                param.span,
                                DisplayParts::symbol(&param.name),
                                param
                                    .type_hint
                                    .as_ref()
                                    .map(|hint| DisplayParts::type_name(hint.display())),
                                DocumentSymbolKind::Field,
                                Vec::new(),
                                SymbolRef::Source(format!("{variant_symbol}.{}", param.name)),
                            )
                        })
                        .collect(),
                    EnumVariantFieldsHint::Record(fields) => fields
                        .iter()
                        .filter_map(|field| {
                            let variant_symbol = format!("{parent_symbol}::{}", variant.name);
                            symbol_from_span(
                                source,
                                field.span,
                                DisplayParts::symbol(&field.name),
                                field
                                    .type_hint
                                    .as_ref()
                                    .map(|hint| DisplayParts::type_name(hint.display())),
                                DocumentSymbolKind::Field,
                                Vec::new(),
                                SymbolRef::Source(format!("{variant_symbol}.{}", field.name)),
                            )
                        })
                        .collect(),
                };
                let variant_symbol = format!("{parent_symbol}::{}", variant.name);
                symbol_from_span(
                    source,
                    variant.span,
                    DisplayParts::symbol(&variant.name),
                    None,
                    DocumentSymbolKind::EnumMember,
                    children,
                    SymbolRef::Source(variant_symbol),
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
                    DisplayParts::symbol(&method.name),
                    Some(signature_detail_parts(&method.signature)),
                    DocumentSymbolKind::Method,
                    Vec::new(),
                    SymbolRef::Source(format!("{parent_symbol}.{}", method.name)),
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
                    DisplayParts::symbol(&method.name),
                    Some(signature_detail_parts(&method.signature)),
                    DocumentSymbolKind::Method,
                    Vec::new(),
                    SymbolRef::Source(format!("{parent_symbol}.{}", method.name)),
                )
            })
            .collect(),
        DeclarationKind::Const | DeclarationKind::Global | DeclarationKind::Function => Vec::new(),
    }
}

fn detail_parts_for_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> Option<DisplayParts> {
    match declaration.kind {
        DeclarationKind::Const => graph.const_metadata(declaration.id).and_then(|metadata| {
            metadata
                .type_hint
                .as_ref()
                .map(|hint| DisplayParts::type_name(hint.display()))
        }),
        DeclarationKind::Global => graph
            .global_metadata(declaration.id)
            .map(|metadata| DisplayParts::type_name(metadata.type_hint.display())),
        DeclarationKind::Function => graph
            .function_signature(declaration.id)
            .map(signature_detail_parts),
        DeclarationKind::Struct
        | DeclarationKind::Enum
        | DeclarationKind::Trait
        | DeclarationKind::Impl => None,
    }
}

fn signature_detail_parts(signature: &FunctionSignature) -> DisplayParts {
    let params = signature.params.iter().map(|param| {
        param.type_hint.as_ref().map_or_else(
            || DisplayParts::symbol(param.name.as_str()),
            |hint| DisplayParts::parameter(&param.name, &hint.display()),
        )
    });
    let return_type = signature.return_type.as_ref().map(|hint| hint.display());
    DisplayParts::signature(params, return_type.as_deref())
}

fn symbol_from_span(
    source: &SourceRecord,
    span: Span,
    name_parts: DisplayParts,
    detail_parts: Option<DisplayParts>,
    kind: DocumentSymbolKind,
    children: Vec<DocumentSymbol>,
    symbol: SymbolRef,
) -> Option<DocumentSymbol> {
    if span.source != source.source_id() {
        return None;
    }
    let name = name_parts.render();
    let detail = detail_parts.as_ref().map(DisplayParts::render);
    let range = diagnostic_range(source.text(), span_range(span)?);
    let selection_range = name_range_in_span(source.text(), span, &name)
        .map_or(range, |range| diagnostic_range(source.text(), range));
    Some(DocumentSymbol {
        name,
        name_parts,
        detail,
        detail_parts,
        kind,
        range,
        selection_range,
        children,
        symbol,
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
        DisplayParts::symbol(&name),
        Some(DisplayParts::type_name(detail)),
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
    let name = DisplayParts::qualified(&member.owner, &member.name);
    let symbol = schema_member_symbol_ref(&member, kind);
    schema_symbol(
        query,
        name,
        Some(DisplayParts::type_name(member.fact.display_name())),
        kind,
        Some(member.owner),
        symbol,
    )
}

fn schema_symbol(
    query: &str,
    name_parts: DisplayParts,
    detail_parts: Option<DisplayParts>,
    kind: DocumentSymbolKind,
    container_name: Option<String>,
    symbol: SymbolRef,
) -> Option<WorkspaceSymbol> {
    let name = name_parts.render();
    let detail = detail_parts.as_ref().map(DisplayParts::render);
    symbol_matches(query, &name).then_some(WorkspaceSymbol {
        name,
        name_parts,
        detail,
        detail_parts,
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
        DisplayPartKind, SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
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
        assert_eq!(player.name_parts().render(), "Player");
        assert_eq!(
            player.name_parts().parts()[0].kind(),
            DisplayPartKind::Symbol
        );
        assert_eq!(
            player.symbol(),
            &SymbolRef::Source("game::main::Player".to_owned())
        );
        assert_eq!(symbol_names(player.children()), ["level", "name"]);
        assert_eq!(
            player.children()[0].symbol(),
            &SymbolRef::Source("game::main::Player.level".to_owned())
        );
        assert_eq!(
            player.children()[0].selection_range().start().line,
            3,
            "struct field selection should point at the field line"
        );

        let reward = symbol(&symbols, "Reward");
        assert_eq!(reward.kind(), DocumentSymbolKind::Enum);
        assert_eq!(
            reward.symbol(),
            &SymbolRef::Source("game::main::Reward".to_owned())
        );
        assert_eq!(symbol_names(reward.children()), ["None", "Coins", "Item"]);
        assert_eq!(
            reward.children()[1].symbol(),
            &SymbolRef::Source("game::main::Reward::Coins".to_owned())
        );
        assert_eq!(symbol_names(reward.children()[1].children()), ["amount"]);
        assert_eq!(
            reward.children()[1].children()[0].symbol(),
            &SymbolRef::Source("game::main::Reward::Coins.amount".to_owned())
        );
        assert_eq!(symbol_names(reward.children()[2].children()), ["id"]);

        let damageable = symbol(&symbols, "Damageable");
        assert_eq!(damageable.kind(), DocumentSymbolKind::Interface);
        assert_eq!(symbol_names(damageable.children()), ["damage"]);
        assert_eq!(
            damageable.children()[0].symbol(),
            &SymbolRef::Source("game::main::Damageable.damage".to_owned())
        );

        let impl_player = symbol(&symbols, "impl Player");
        assert_eq!(impl_player.kind(), DocumentSymbolKind::Object);
        assert_eq!(symbol_names(impl_player.children()), ["heal"]);
        assert_eq!(
            impl_player.children()[0].symbol(),
            &SymbolRef::Source("game::main::impl Player.heal".to_owned())
        );

        let main = symbol(&symbols, "main");
        assert_eq!(main.detail(), Some("(amount: i64) -> i64"));
        assert_eq!(
            main.detail_parts().map(DisplayParts::render).as_deref(),
            Some("(amount: i64) -> i64")
        );
        assert!(main.detail_parts().is_some_and(|parts| {
            parts
                .parts()
                .iter()
                .any(|part| part.kind() == DisplayPartKind::Parameter)
        }));
        assert_eq!(main.kind(), DocumentSymbolKind::Function);
        assert_eq!(
            main.symbol(),
            &SymbolRef::Source("game::main::main".to_owned())
        );
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
                    && symbol.name_parts().render() == "game::reward::grant"
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
                && symbol
                    .detail_parts()
                    .is_some_and(|parts| parts.parts()[0].kind() == DisplayPartKind::Type)
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
