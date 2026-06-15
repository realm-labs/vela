use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, TextRange,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CallHierarchyItem {
    name: String,
    document_id: DocumentId,
    range: DiagnosticRange,
    selection_range: DiagnosticRange,
}

impl CallHierarchyItem {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        document_id: DocumentId,
        range: DiagnosticRange,
        selection_range: DiagnosticRange,
    ) -> Self {
        Self {
            name: name.into(),
            document_id,
            range,
            selection_range,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub const fn range(&self) -> DiagnosticRange {
        self.range
    }

    #[must_use]
    pub const fn selection_range(&self) -> DiagnosticRange {
        self.selection_range
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IncomingCall {
    from: CallHierarchyItem,
    from_ranges: Vec<DiagnosticRange>,
}

impl IncomingCall {
    #[must_use]
    pub fn from(&self) -> &CallHierarchyItem {
        &self.from
    }

    #[must_use]
    pub fn from_ranges(&self) -> &[DiagnosticRange] {
        &self.from_ranges
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OutgoingCall {
    to: CallHierarchyItem,
    from_ranges: Vec<DiagnosticRange>,
}

impl OutgoingCall {
    #[must_use]
    pub fn to(&self) -> &CallHierarchyItem {
        &self.to
    }

    #[must_use]
    pub fn from_ranges(&self) -> &[DiagnosticRange] {
        &self.from_ranges
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn prepare_call_hierarchy(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Vec<CallHierarchyItem> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let Some(token) = call_hierarchy_token_at(source.text(), position) else {
            return Vec::new();
        };
        let source_id = source.source_id();
        let Ok(offset) = u32::try_from(token.range.start) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();

        for declaration in graph.declarations() {
            if declaration.span.source != source_id || !declaration.span.contains(offset) {
                continue;
            }
            if declaration.kind == DeclarationKind::Function
                && token_text(source.text(), token.range) == Some(declaration.name.as_str())
                && let Some(item) = self.call_hierarchy_item_for_declaration(declaration)
            {
                return vec![item];
            }
            let Some(bindings) = graph.bindings(declaration.id) else {
                continue;
            };
            if let Some(target) = declaration_call_target(bindings, &token)
                && let Some(target) = graph.declaration(target)
                && target.kind == DeclarationKind::Function
                && let Some(item) = self.call_hierarchy_item_for_declaration(target)
            {
                return vec![item];
            }
        }

        Vec::new()
    }

    #[must_use]
    pub fn incoming_calls(&self, item: &CallHierarchyItem) -> Vec<IncomingCall> {
        let Some(target) = self.call_hierarchy_declaration_for_item(item) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let mut calls = Vec::new();

        for caller in graph.declarations() {
            if caller.kind != DeclarationKind::Function {
                continue;
            }
            let Some(bindings) = graph.bindings(caller.id) else {
                continue;
            };
            let ranges = self.call_ranges_to(bindings, target);
            if ranges.is_empty() {
                continue;
            }
            if let Some(from) = self.call_hierarchy_item_for_declaration(caller) {
                calls.push(IncomingCall {
                    from,
                    from_ranges: ranges,
                });
            }
        }

        calls.sort_by_key(|call| {
            let start = call.from.selection_range.start();
            (
                call.from.document_id.as_str().to_owned(),
                start.line,
                start.character,
            )
        });
        calls
    }

    #[must_use]
    pub fn outgoing_calls(&self, item: &CallHierarchyItem) -> Vec<OutgoingCall> {
        let Some(caller) = self.call_hierarchy_declaration_for_item(item) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let Some(bindings) = graph.bindings(caller) else {
            return Vec::new();
        };
        let mut calls = Vec::<OutgoingCall>::new();

        for (target, range) in self.resolved_call_ranges(bindings) {
            let Some(target_declaration) = graph.declaration(target) else {
                continue;
            };
            if target_declaration.kind != DeclarationKind::Function {
                continue;
            }
            let Some(to) = self.call_hierarchy_item_for_declaration(target_declaration) else {
                continue;
            };
            if let Some(existing) = calls.iter_mut().find(|call| call.to == to) {
                existing.from_ranges.push(range);
            } else {
                calls.push(OutgoingCall {
                    to,
                    from_ranges: vec![range],
                });
            }
        }

        calls.sort_by_key(|call| {
            let start = call.to.selection_range.start();
            (
                call.to.document_id.as_str().to_owned(),
                start.line,
                start.character,
            )
        });
        calls
    }

    fn call_ranges_to(&self, bindings: &BindingMap, target: HirDeclId) -> Vec<DiagnosticRange> {
        self.resolved_call_ranges(bindings)
            .into_iter()
            .filter_map(|(resolved, range)| (resolved == target).then_some(range))
            .collect()
    }

    fn resolved_call_ranges(&self, bindings: &BindingMap) -> Vec<(HirDeclId, DiagnosticRange)> {
        bindings
            .resolutions()
            .filter_map(|(expression, resolution)| match resolution {
                BindingResolution::Declaration(target) => {
                    let expression = bindings.expression(expression)?;
                    self.call_range_for_expression(expression.span)
                        .map(|range| (*target, range))
                }
                BindingResolution::Local(_)
                | BindingResolution::Import(_)
                | BindingResolution::QualifiedPath(_) => None,
            })
            .collect()
    }

    fn call_range_for_expression(&self, span: Span) -> Option<DiagnosticRange> {
        let source = self.source_record_for_call_hierarchy(span.source)?;
        let range = span_text_range(span)?;
        is_call_callee(source.text(), range).then(|| diagnostic_range(source.text(), range))
    }

    fn call_hierarchy_declaration_for_item(&self, item: &CallHierarchyItem) -> Option<HirDeclId> {
        let source = self.source_db().records().get(item.document_id())?;
        let graph = self.hir_db().graph();
        graph
            .declarations()
            .find(|declaration| {
                declaration.kind == DeclarationKind::Function
                    && declaration.span.source == source.source_id()
                    && declaration.name == item.name()
                    && self
                        .call_hierarchy_item_for_declaration(declaration)
                        .is_some_and(|candidate| candidate.selection_range == item.selection_range)
            })
            .map(|declaration| declaration.id)
    }

    fn call_hierarchy_item_for_declaration(
        &self,
        declaration: &Declaration,
    ) -> Option<CallHierarchyItem> {
        let source = self.source_record_for_call_hierarchy(declaration.span.source)?;
        let span_range = span_text_range(declaration.span)?;
        let name_range =
            name_range_in_text(source.text(), span_range, &declaration.name).unwrap_or(span_range);
        Some(CallHierarchyItem::new(
            declaration.name.clone(),
            source.document_id().clone(),
            diagnostic_range(source.text(), span_range),
            diagnostic_range(source.text(), name_range),
        ))
    }

    fn source_record_for_call_hierarchy(
        &self,
        source_id: SourceId,
    ) -> Option<&crate::SourceRecord> {
        self.source_db()
            .records()
            .values()
            .find(|record| record.source_id() == source_id)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CallHierarchyToken {
    range: TextRange,
}

fn declaration_call_target(bindings: &BindingMap, token: &CallHierarchyToken) -> Option<HirDeclId> {
    let resolution = bindings
        .resolutions()
        .filter_map(|(expression, resolution)| {
            let expression = bindings.expression(expression)?;
            let start = usize::try_from(expression.span.start).ok()?;
            let end = usize::try_from(expression.span.end).ok()?;
            (start <= token.range.start && token.range.end <= end)
                .then_some((end.saturating_sub(start), resolution))
        })
        .min_by_key(|(len, _)| *len)?
        .1;

    match resolution {
        BindingResolution::Declaration(declaration) => Some(*declaration),
        BindingResolution::Local(_)
        | BindingResolution::Import(_)
        | BindingResolution::QualifiedPath(_) => None,
    }
}

fn call_hierarchy_token_at(text: &str, position: Position) -> Option<CallHierarchyToken> {
    let offset = LineIndex::new(text).offset(position);
    let range = identifier_range_at(text, offset)?;
    Some(CallHierarchyToken { range })
}

fn identifier_range_at(text: &str, offset: usize) -> Option<TextRange> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| TextRange::new(start, end))
}

fn is_call_callee(text: &str, range: TextRange) -> bool {
    text.get(range.end..)
        .is_some_and(|suffix| suffix.trim_start().starts_with('('))
}

fn diagnostic_range(text: &str, range: TextRange) -> DiagnosticRange {
    let line_index = LineIndex::new(text);
    DiagnosticRange::new(
        line_index.position(range.start),
        line_index.position(range.end),
    )
}

fn span_text_range(span: Span) -> Option<TextRange> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    Some(TextRange::new(start, end))
}

fn name_range_in_text(text: &str, range: TextRange, name: &str) -> Option<TextRange> {
    let slice = text.get(range.start..range.end)?;
    slice.match_indices(name).find_map(|(offset, matched)| {
        let start = range.start + offset;
        let end = start + matched.len();
        is_identifier_boundary(text, start, end).then(|| TextRange::new(start, end))
    })
}

fn is_identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn token_text(text: &str, range: TextRange) -> Option<&str> {
    text.get(range.start..range.end)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn call_hierarchy_uses_resolved_call_graph() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let helper = DocumentId::from("/workspace/scripts/game/reward.vela");
        let main_text = "\
use game::reward::grant
pub fn main(amount: i64) -> i64 {
    let first = grant(amount)
    return grant(first)
}";
        let helper_text = "pub fn grant(amount: i64) -> i64 { return amount }";
        let databases = databases_for(vec![
            SourceFileSnapshot::new(main.clone(), main_text),
            SourceFileSnapshot::new(helper.clone(), helper_text),
        ]);

        let prepared = databases.prepare_call_hierarchy(
            &helper,
            Position::new(0, helper_text.find("grant").expect("grant declaration")),
        );

        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].name(), "grant");
        assert_eq!(prepared[0].document_id(), &helper);

        let incoming = databases.incoming_calls(&prepared[0]);
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from().name(), "main");
        assert_eq!(incoming[0].from().document_id(), &main);
        assert_eq!(incoming[0].from_ranges().len(), 2);
        assert_range(
            incoming[0].from_ranges(),
            2,
            line(main_text, 2).find("grant").expect("first call"),
        );
        assert_range(
            incoming[0].from_ranges(),
            3,
            line(main_text, 3).find("grant").expect("second call"),
        );

        let main_item = databases
            .prepare_call_hierarchy(
                &main,
                Position::new(
                    1,
                    line(main_text, 1).find("main").expect("main declaration"),
                ),
            )
            .pop()
            .expect("main should prepare a call hierarchy item");
        let outgoing = databases.outgoing_calls(&main_item);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to().name(), "grant");
        assert_eq!(outgoing[0].to().document_id(), &helper);
        assert_eq!(outgoing[0].from_ranges().len(), 2);
    }

    fn assert_range(ranges: &[DiagnosticRange], line: usize, character: usize) {
        assert!(
            ranges.iter().any(|range| {
                range.start().line == line && range.start().character == character
            }),
            "{ranges:?}"
        );
    }

    fn line(text: &str, line: usize) -> &str {
        text.lines().nth(line).expect("line should exist")
    }

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }
}
