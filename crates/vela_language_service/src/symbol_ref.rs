use vela_common::Span;
use vela_hir::binding::LocalBinding;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;
use vela_package::ModuleKey;

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
    pub fn local_for_binding(binding: &LocalBinding, document_id: DocumentId) -> Self {
        let Some(range) = span_text_range(binding.span) else {
            return Self::local(binding.name.clone());
        };
        Self::local_at(binding.name.clone(), document_id, range)
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

pub(crate) fn source_symbol(name: impl Into<String>) -> SymbolRef {
    SymbolRef::Source(name.into())
}

pub(crate) fn source_symbol_for_declaration(
    graph: &ModuleGraph,
    declaration: &Declaration,
) -> SymbolRef {
    source_symbol(qualified_source_declaration_name(graph, declaration))
}

pub(crate) fn source_module_symbol(key: &ModuleKey) -> SymbolRef {
    source_symbol(format!("{}::{}", key.package, key.path.join()))
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
    Some(source_child_symbol(&owner, member))
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
    Some(source_child_symbol(&owner, method))
}

pub(crate) fn source_impl_owner_matches(graph: &ModuleGraph, id: HirDeclId, owner: &str) -> bool {
    let Some(declaration) = graph.declaration(id) else {
        return false;
    };
    let Some(metadata) = graph.impl_metadata(id) else {
        return false;
    };
    let Some(path) = graph.expand_import_path(declaration.module, &metadata.target_path) else {
        return false;
    };
    let Some(current) = graph.module_key(declaration.module) else {
        return false;
    };
    let target = [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ]
    .into_iter()
    .find_map(|kind| graph.declaration_by_type_path(&path, current, kind));
    let Some(target) = target else {
        // A known registry receiver may have script extension methods. Its
        // complete expanded name owns that impl; terminal-name guesses do not.
        return path.join("::") == owner;
    };
    if !matches!(
        target.kind,
        DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
    ) || target.module != declaration.module
        && target.visibility != vela_hir::module_graph::Visibility::Public
    {
        return false;
    }
    if qualified_source_declaration_name(graph, target) == owner {
        return true;
    }
    target.name == owner
        && graph
            .declarations()
            .filter(|candidate| candidate.kind == target.kind && candidate.name == owner)
            .count()
            == 1
}

pub(crate) fn source_enum_variant_symbol(
    graph: &ModuleGraph,
    declaration: HirDeclId,
    variant: &str,
) -> Option<SymbolRef> {
    let SymbolRef::Source(owner) = source_symbol_for_declaration_id(graph, declaration)? else {
        return None;
    };
    Some(source_variant_symbol(&owner, variant))
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
    Some(source_child_symbol(&variant, field))
}

pub(crate) fn source_child_symbol(owner: &str, member: &str) -> SymbolRef {
    SymbolRef::Source(format!("{owner}.{member}"))
}

pub(crate) fn source_variant_symbol(owner: &str, variant: &str) -> SymbolRef {
    SymbolRef::Source(format!("{owner}::{variant}"))
}

pub(crate) fn schema_symbol(name: impl Into<String>) -> SymbolRef {
    SymbolRef::Schema(name.into())
}

pub(crate) fn schema_member_symbol(owner: &str, member: &str) -> SymbolRef {
    SymbolRef::Schema(format!("{owner}.{member}"))
}

pub(crate) fn schema_variant_symbol(owner: &str, variant: &str) -> SymbolRef {
    SymbolRef::Schema(format!("{owner}::{variant}"))
}

pub(crate) fn builtin_symbol(name: impl Into<String>) -> SymbolRef {
    SymbolRef::Builtin(name.into())
}

pub(crate) fn builtin_member_symbol(owner: &str, member: &str) -> SymbolRef {
    SymbolRef::Builtin(format!("{owner}.{member}"))
}

pub(crate) fn source_impl_has_owner(
    graph: &ModuleGraph,
    declaration: &vela_hir::module_graph::Declaration,
    owner: Option<vela_hir::ids::HirDeclId>,
) -> bool {
    let Some(owner) = owner else {
        return true;
    };
    let Some(metadata) = graph.impl_metadata(declaration.id) else {
        return false;
    };
    let Some(path) = graph.expand_import_path(declaration.module, &metadata.target_path) else {
        return false;
    };
    [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
    ]
    .into_iter()
    .any(|kind| {
        graph
            .resolve_visible_declaration_path(declaration.module, &path, kind)
            .is_some_and(|target| target.id == owner)
    })
}
