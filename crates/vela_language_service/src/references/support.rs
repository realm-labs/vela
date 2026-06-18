use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::module_graph::{Declaration, ModuleGraph};

use crate::{DiagnosticRange, LineIndex, TextRange, symbol_ref::qualified_source_declaration_name};

use super::ReferenceKind;

pub(super) fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
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

pub(super) fn declaration_name_matches(
    graph: &ModuleGraph,
    declaration: &Declaration,
    owner: &str,
) -> bool {
    declaration.name == owner || qualified_source_declaration_name(graph, declaration) == owner
}

pub(super) fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

pub(super) fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

pub(super) fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

pub(super) fn last_name_range_in_text(
    text: &str,
    range: TextRange,
    name: &str,
) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.rmatch_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

pub(super) fn is_identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

pub(super) fn token_text(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
}

pub(super) fn resolved_use_reference_kind(text: &str, range: TextRange) -> ReferenceKind {
    if is_call_callee(text, range) {
        ReferenceKind::Call
    } else if is_assignment_target(text, range) {
        ReferenceKind::Write
    } else {
        ReferenceKind::Read
    }
}

pub(super) fn is_call_callee(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .is_some_and(|suffix| suffix.trim_start().starts_with('('))
}

fn is_assignment_target(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .map(str::trim_start)
        .is_some_and(|suffix| {
            suffix.starts_with("+=")
                || suffix.starts_with("-=")
                || suffix.starts_with("*=")
                || suffix.starts_with("/=")
                || suffix.starts_with("%=")
                || (suffix.starts_with('=')
                    && !suffix.starts_with("==")
                    && !suffix.starts_with("=>"))
        })
}

pub(super) fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
