use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::BindingMap;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{LanguageServiceDatabases, TextRange};

use super::{
    Reference, ReferenceKind, ReferenceToken, diagnostic_range, is_call_callee,
    is_identifier_boundary, is_identifier_continue, member_receiver_range, record_owner_names,
    span_text_range, token_text, type_fact_for_resolution,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MethodReferenceTarget {
    owner: HirDeclId,
    method: String,
}

pub(super) fn script_method_references(
    databases: &LanguageServiceDatabases,
    target: &MethodReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_script_method_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(script_method_use_references_for_source(
            graph, &facts, source, target,
        ));
    }

    references.sort_by_key(|reference| {
        let start = reference.range().start();
        (
            reference.document_id().as_str().to_owned(),
            start.line,
            start.character,
            reference.kind(),
        )
    });
    references
}

pub(super) fn script_method_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<MethodReferenceTarget> {
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
                return Some(MethodReferenceTarget {
                    owner: declaration.id,
                    method: method.name.clone(),
                });
            }
        }
    }
    None
}

pub(super) fn script_method_use_target(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    token: &ReferenceToken,
) -> Option<MethodReferenceTarget> {
    let method = token_text(text, token.range)?;
    if !is_call_callee(text, token.range) {
        return None;
    }
    script_method_target_for_member(graph, facts, text, source_id, bindings, method, token.range)
}

fn reference_for_script_method_declaration(
    databases: &LanguageServiceDatabases,
    target: &MethodReferenceTarget,
) -> Option<Reference> {
    let graph = databases.hir_db().graph();
    let metadata = graph.impl_metadata(target.owner)?;
    let method = metadata
        .methods
        .iter()
        .find(|method| method.name == target.method)?;
    let declaration = graph.declaration(target.owner)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == declaration.span.source)?;
    let span_range = span_text_range(declaration.span)?;
    let name_range =
        method_name_range_in_text(source.text(), span_range, &method.name).unwrap_or(span_range);
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), name_range),
        kind: ReferenceKind::Declaration,
    })
}

fn script_method_use_references_for_source(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    source: &crate::SourceRecord,
    target: &MethodReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let source_id = source.source_id();
    let text = source.text();
    for range in member_method_ranges(source_id, text, &target.method) {
        let Some(start) = u32::try_from(range.start).ok() else {
            continue;
        };
        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                continue;
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if script_method_target_for_member(
                graph,
                facts,
                text,
                source_id,
                bindings,
                &target.method,
                range,
            )
            .as_ref()
                == Some(target)
            {
                references.push(Reference {
                    document_id: source.document_id().clone(),
                    range: diagnostic_range(text, range),
                    kind: ReferenceKind::Call,
                });
                break;
            }
        }
    }
    references
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

fn script_method_target_for_member(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    method: &str,
    member_range: TextRange,
) -> Option<MethodReferenceTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = type_fact_for_resolution(resolution, facts)?;
    let owner = script_method_owner(graph, &receiver, method)?;
    Some(MethodReferenceTarget {
        owner,
        method: method.to_owned(),
    })
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
