use std::collections::BTreeMap;

use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{DocumentId, LanguageServiceDatabases, TextRange};

use super::{
    RenameToken, TextEdit, WorkspaceEdit, diagnostic_range, document_text_edit_for_rename,
    name_range_in_text, qualified_declaration_path, span_text_range, token_text,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ScriptFieldRenameTarget {
    pub(super) owner: HirDeclId,
    pub(super) field: String,
    pub(super) token: RenameToken,
}

pub(super) fn rename_script_field(
    databases: &LanguageServiceDatabases,
    target: ScriptFieldRenameTarget,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let graph = databases.hir_db().graph();
    if script_field_name_conflicts(graph, &target, new_name) {
        return None;
    }

    let mut edits_by_document = BTreeMap::<DocumentId, Vec<TextEdit>>::new();
    push_script_field_declaration_edit(databases, &target, new_name, &mut edits_by_document)?;
    push_script_field_use_edits(databases, &target, new_name, &mut edits_by_document);

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

pub(super) fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &RenameToken,
) -> Option<ScriptFieldRenameTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Struct
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.struct_shape(declaration.id)?;
        for field in &shape.fields {
            let span_range = span_text_range(field.span)?;
            let name_range = name_range_in_text(text, span_range, &field.name)?;
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(ScriptFieldRenameTarget {
                    owner: declaration.id,
                    field: field.name.clone(),
                    token: token.clone(),
                });
            }
        }
    }
    None
}

pub(super) fn script_field_use_target(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    token: &RenameToken,
) -> Option<ScriptFieldRenameTarget> {
    let field = token_text(text, token.range)?;
    let target = script_field_target_for_member(
        graph,
        facts,
        text,
        source_id,
        bindings,
        field,
        token.range,
    )?;
    Some(ScriptFieldRenameTarget {
        owner: target.owner,
        field: target.field,
        token: token.clone(),
    })
}

fn push_script_field_declaration_edit(
    databases: &LanguageServiceDatabases,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) -> Option<()> {
    let graph = databases.hir_db().graph();
    let field = graph
        .struct_shape(target.owner)?
        .fields
        .iter()
        .find(|field| field.name == target.field)?;
    let source = databases.source_record_for_rename(field.span.source)?;
    let span_range = span_text_range(field.span)?;
    let range = name_range_in_text(source.text(), span_range, &field.name)?;
    edits_by_document
        .entry(source.document_id().clone())
        .or_default()
        .push(TextEdit {
            range: diagnostic_range(source.text(), range),
            new_text: new_name.to_owned(),
        });
    Some(())
}

fn push_script_field_use_edits(
    databases: &LanguageServiceDatabases,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
    edits_by_document: &mut BTreeMap<DocumentId, Vec<TextEdit>>,
) {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    for source in databases.source_db().records().values() {
        let source_id = source.source_id();
        let text = source.text();
        for range in member_field_ranges(source_id, text, &target.field) {
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
                if script_field_target_for_member(
                    graph,
                    &facts,
                    text,
                    source_id,
                    bindings,
                    &target.field,
                    range,
                )
                .is_some_and(|found| found.owner == target.owner && found.field == target.field)
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
struct ScriptFieldTarget {
    owner: HirDeclId,
    field: String,
}

fn script_field_target_for_member(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    field: &str,
    member_range: TextRange,
) -> Option<ScriptFieldTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = type_fact_for_resolution(resolution, facts)?;
    let owner = script_field_owner(graph, &receiver, field)?;
    Some(ScriptFieldTarget {
        owner,
        field: field.to_owned(),
    })
}

fn script_field_owner(graph: &ModuleGraph, receiver: &TypeFact, field: &str) -> Option<HirDeclId> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct {
            return None;
        }
        let matches_owner = owner_names
            .iter()
            .any(|owner| declaration_name_matches(graph, declaration, owner));
        let has_field = graph
            .struct_shape(declaration.id)
            .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field));
        (matches_owner && has_field).then_some(declaration.id)
    })
}

fn script_field_name_conflicts(
    graph: &ModuleGraph,
    target: &ScriptFieldRenameTarget,
    new_name: &str,
) -> bool {
    graph.struct_shape(target.owner).is_some_and(|shape| {
        shape
            .fields
            .iter()
            .any(|field| field.name == new_name && field.name != target.field)
    })
}

fn member_field_ranges(source_id: SourceId, text: &str, field: &str) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) if name == field => {
                let range = span_text_range(token.span)?;
                member_receiver_range(text, range.start).map(|_| range)
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

fn declaration_name_matches(graph: &ModuleGraph, declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner || qualified_declaration_path(graph, declaration).join("::") == owner
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
