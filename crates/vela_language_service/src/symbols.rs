use vela_common::Span;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, SourceRecord, TextRange,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DocumentSymbolKind {
    Constant,
    Enum,
    EnumMember,
    Field,
    Function,
    Interface,
    Method,
    Object,
    Struct,
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
}

fn symbol_from_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
    source: &SourceRecord,
) -> Option<DocumentSymbol> {
    let kind = match declaration.kind {
        DeclarationKind::Const => DocumentSymbolKind::Constant,
        DeclarationKind::Global => DocumentSymbolKind::Variable,
        DeclarationKind::Function => DocumentSymbolKind::Function,
        DeclarationKind::Struct => DocumentSymbolKind::Struct,
        DeclarationKind::Enum => DocumentSymbolKind::Enum,
        DeclarationKind::Trait => DocumentSymbolKind::Interface,
        DeclarationKind::Impl => DocumentSymbolKind::Object,
    };
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
    let params = signature
        .params
        .iter()
        .map(|param| {
            param.type_hint.as_ref().map_or_else(
                || param.name.clone(),
                |hint| format!("{}: {}", param.name, hint.display()),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    signature.return_type.as_ref().map_or_else(
        || format!("({params})"),
        |return_type| format!("({params}) -> {}", return_type.display()),
    )
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
