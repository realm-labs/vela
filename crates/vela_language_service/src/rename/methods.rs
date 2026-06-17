use std::collections::BTreeMap;

use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{DocumentId, LanguageServiceDatabases, TextRange};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, document_text_edit_for_rename,
    is_identifier_boundary, span_text_range, token_text,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ScriptMethodRenameTarget {
    pub(super) owner: HirDeclId,
    pub(super) method: String,
    pub(super) token: RenameToken,
}

pub(super) fn rename_script_method(
    databases: &LanguageServiceDatabases,
    target: ScriptMethodRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let graph = databases.hir_db().graph();
    if script_method_name_conflicts(graph, &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_script_method_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_script_method_use_edits(databases, &target, new_name, &mut edits_by_document);

    let document_edits = edits_by_document
        .into_iter()
        .map(|(document_id, mut edits)| {
            edits.sort_by_key(|edit| {
                let start = edit.range.start();
                (start.line, start.character)
            });
            edits.dedup();
            document_text_edit_for_rename(databases, document_id, edits)
        })
        .collect::<Vec<_>>();

    Some(WorkspaceEdit {
        document_edits,
        risks: Vec::new(),
    })
}

pub(super) fn script_method_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &RenameToken,
) -> Option<ScriptMethodRenameTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Impl
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        for method in &metadata.methods {
            let span_range = span_text_range(declaration.span)?;
            let name_range = method_name_range_in_text(text, span_range, &method.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(ScriptMethodRenameTarget {
                    owner: declaration.id,
                    method: method.name.clone(),
                    token: token.clone(),
                });
            }
        }
    }
    None
}

pub(super) fn script_method_target_for_self_receiver(
    graph: &ModuleGraph,
    scope_owner: HirDeclId,
    method: &str,
    token: &RenameToken,
) -> Option<ScriptMethodRenameTarget> {
    script_method_exists_in_inherent_impl(graph, scope_owner, method).then(|| {
        ScriptMethodRenameTarget {
            owner: scope_owner,
            method: method.to_owned(),
            token: token.clone(),
        }
    })
}

pub(super) fn script_method_target_for_receiver_fact(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
    token: &RenameToken,
) -> Option<ScriptMethodRenameTarget> {
    let owner = script_method_owner(graph, receiver, method)?;
    Some(ScriptMethodRenameTarget {
        owner,
        method: method.to_owned(),
        token: token.clone(),
    })
}

fn push_script_method_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &ScriptMethodRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let graph = databases.hir_db().graph();
    let metadata = graph.impl_metadata(target.owner)?;
    let method = metadata
        .methods
        .iter()
        .find(|method| method.name == target.method)?;
    let declaration = graph.declaration(target.owner)?;
    let source = databases.source_record_for_rename(declaration.span.source)?;
    let span_range = span_text_range(declaration.span)?;
    let range = method_name_range_in_text(source.text(), span_range, &method.name)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_script_method_use_edits(
    databases: &LanguageServiceDatabases,
    target: &ScriptMethodRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    for source in databases.source_db().records().values() {
        let source_id = source.source_id();
        let text = source.text();
        let lookup = MethodLookup {
            graph,
            facts: &facts,
            text,
            source_id,
        };
        for range in member_method_ranges(source_id, text, &target.method) {
            let Some(start) = u32::try_from(range.start).ok() else {
                continue;
            };
            for declaration in graph.declarations() {
                if declaration.span.source != source_id || !declaration.span.contains(start) {
                    continue;
                }
                if script_method_target_for_member(
                    lookup,
                    declaration.id,
                    graph.bindings(declaration.id),
                    &target.method,
                    range,
                )
                .is_some_and(|found| found.owner == target.owner && found.method == target.method)
                {
                    edits_by_document
                        .entry(source.document_id().clone())
                        .or_default()
                        .push(TextEdit {
                            range: diagnostic_range(text, range),
                            new_text: new_name.to_owned(),
                        });
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ScriptMethodTarget {
    owner: HirDeclId,
    method: String,
}

#[derive(Clone, Copy)]
struct MethodLookup<'a> {
    graph: &'a ModuleGraph,
    facts: &'a AnalysisFacts,
    text: &'a str,
    source_id: SourceId,
}

fn script_method_target_for_member(
    lookup: MethodLookup<'_>,
    scope_owner: HirDeclId,
    bindings: Option<&BindingMap>,
    method: &str,
    member_range: TextRange,
) -> Option<ScriptMethodTarget> {
    let receiver = member_receiver_range(lookup.text, member_range.start)?;
    if token_text(lookup.text, receiver) == Some("self")
        && script_method_exists_in_inherent_impl(lookup.graph, scope_owner, method)
    {
        return Some(ScriptMethodTarget {
            owner: scope_owner,
            method: method.to_owned(),
        });
    }
    let bindings = bindings?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(lookup.source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = type_fact_for_resolution(resolution, lookup.facts)?;
    let owner = script_method_owner(lookup.graph, &receiver, method)?;
    Some(ScriptMethodTarget {
        owner,
        method: method.to_owned(),
    })
}

fn script_method_exists_in_inherent_impl(
    graph: &ModuleGraph,
    owner: HirDeclId,
    method: &str,
) -> bool {
    let Some(metadata) = graph.impl_metadata(owner) else {
        return false;
    };
    matches!(metadata.kind, ImplMetadataKind::Inherent)
        && metadata.methods.iter().any(|entry| entry.name == method)
}

fn script_method_owner(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
) -> Option<HirDeclId> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
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
        let has_method = metadata.methods.iter().any(|entry| entry.name == method);
        (matches_owner && has_method).then_some(declaration.id)
    })
}

fn script_method_name_conflicts(
    graph: &ModuleGraph,
    target: &ScriptMethodRenameTarget,
    new_name: &str,
) -> bool {
    graph.impl_metadata(target.owner).is_some_and(|metadata| {
        metadata
            .methods
            .iter()
            .any(|method| method.name == new_name && method.name != target.method)
    })
}

fn member_method_ranges(source_id: SourceId, text: &str, method: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == method => {
                let range = span_text_range(token.span)?;
                (is_call_callee(text, range) && member_receiver_range(text, range.start).is_some())
                    .then_some(range)
            }
            TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::String(_)
            | TokenKind::InterpolatedString(_)
            | TokenKind::Bytes(_)
            | TokenKind::Keyword(_)
            | TokenKind::Symbol(_)
            | TokenKind::Eof => None,
        })
        .collect()
}

fn method_name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        (is_identifier_boundary(text, start, end) && preceded_by_fn_keyword(text, start))
            .then(|| TextRange::new(start, end))
    })
}

fn preceded_by_fn_keyword(text: &str, start: usize) -> bool {
    let Some(before_name) = text.get(..start).map(str::trim_end) else {
        return false;
    };
    let end = before_name.len();
    let word_start = before_name
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    if before_name.get(word_start..end) != Some("fn") {
        return false;
    }
    before_name
        .get(..word_start)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|ch| !is_identifier_continue(ch))
}

fn is_call_callee(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .is_some_and(|suffix| suffix.trim_start().starts_with('('))
}

fn member_receiver_range(text: &str, member_start: usize) -> Option<TextRange> {
    let before_member = text.get(..member_start)?.trim_end();
    let before_dot = before_member.strip_suffix('.')?.trim_end();
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    facts: &AnalysisFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => facts
            .local(*local)
            .cloned()
            .filter(|fact| !matches!(fact, TypeFact::Unknown)),
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
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

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
