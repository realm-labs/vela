use vela_common::Span;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::{DocumentId, TextRange};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SymbolRef {
    Source(String),
    Schema(String),
    Builtin(String),
    Local(LocalSymbolRef),
}

impl SymbolRef {
    #[must_use]
    pub fn local(name: impl Into<String>) -> Self {
        Self::Local(LocalSymbolRef::new(name))
    }

    #[must_use]
    pub fn local_at(name: impl Into<String>, document_id: DocumentId, range: TextRange) -> Self {
        Self::Local(LocalSymbolRef::with_location(name, document_id, range))
    }

    #[must_use]
    pub fn local_from_span(
        name: impl Into<String>,
        document_id: DocumentId,
        source_text: &str,
        span: Span,
    ) -> Self {
        let name = name.into();
        let Some(span_range) = span_text_range(span) else {
            return Self::local(name);
        };
        let Some(name_range) = name_range_in_text(source_text, span_range, &name) else {
            return Self::local(name);
        };
        Self::local_at(name, document_id, name_range)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalSymbolRef {
    name: String,
    document_id: Option<DocumentId>,
    range: Option<TextRange>,
}

impl LocalSymbolRef {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            document_id: None,
            range: None,
        }
    }

    #[must_use]
    pub fn with_location(
        name: impl Into<String>,
        document_id: DocumentId,
        range: TextRange,
    ) -> Self {
        Self {
            name: name.into(),
            document_id: Some(document_id),
            range: Some(range),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn document_id(&self) -> Option<&DocumentId> {
        self.document_id.as_ref()
    }

    #[must_use]
    pub const fn range(&self) -> Option<TextRange> {
        self.range
    }
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    let relative = slice.find(name)?;
    let start = range.start + relative;
    Some(TextRange::new(start, start + name.len()))
}

pub(crate) fn qualified_source_declaration_path(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> Vec<String> {
    graph
        .module_path(declaration.module)
        .map(|path| {
            path.segments()
                .iter()
                .chain(std::iter::once(&declaration.name))
                .cloned()
                .collect()
        })
        .unwrap_or_else(|| vec![declaration.name.clone()])
}

pub(crate) fn qualified_source_declaration_name(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> String {
    qualified_source_declaration_path(graph, declaration).join("::")
}

pub(crate) fn source_symbol_for_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> SymbolRef {
    SymbolRef::Source(qualified_source_declaration_name(graph, declaration))
}

pub(crate) fn source_symbol_for_declaration_id(
    graph: &ModuleGraph,
    declaration: HirDeclId,
) -> Option<SymbolRef> {
    graph
        .declaration(declaration)
        .map(|declaration| source_symbol_for_declaration(graph, declaration))
}

pub(crate) fn source_member_symbol(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    member: &str,
) -> Option<SymbolRef> {
    let SymbolRef::Source(owner) = source_symbol_for_declaration_id(graph, declaration)? else {
        return None;
    };
    Some(SymbolRef::Source(format!("{owner}.{member}")))
}

pub(crate) fn source_impl_method_symbol(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    method: &str,
) -> Option<SymbolRef> {
    let declaration = graph.declaration(declaration)?;
    let metadata = graph.impl_metadata(declaration.id)?;
    let owner = match &metadata.kind {
        ImplMetadataKind::Inherent => metadata
            .target_path
            .last()
            .map(|target| {
                graph
                    .module_path(declaration.module)
                    .map(|path| {
                        let module = path.join();
                        if module.is_empty() {
                            target.clone()
                        } else {
                            format!("{module}::{target}")
                        }
                    })
                    .unwrap_or_else(|| target.clone())
            })
            .unwrap_or_else(|| qualified_source_declaration_name(graph, declaration)),
        ImplMetadataKind::Trait { trait_path } => {
            let trait_name = trait_path.join("::");
            let target = metadata.target_path.join("::");
            format!("{trait_name} for {target}")
        }
    };
    Some(SymbolRef::Source(format!("{owner}.{method}")))
}

pub(crate) fn source_enum_variant_symbol(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    variant: &str,
) -> Option<SymbolRef> {
    let SymbolRef::Source(owner) = source_symbol_for_declaration_id(graph, declaration)? else {
        return None;
    };
    Some(SymbolRef::Source(format!("{owner}::{variant}")))
}

pub(crate) fn source_variant_field_symbol(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    variant: &str,
    field: &str,
) -> Option<SymbolRef> {
    let SymbolRef::Source(variant) = source_enum_variant_symbol(graph, declaration, variant)?
    else {
        return None;
    };
    Some(SymbolRef::Source(format!("{variant}.{field}")))
}
