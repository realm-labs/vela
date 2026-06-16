use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::{HirDeclId, HirNodeId};
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

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
        let facts = AnalysisFacts::from_module_graph(graph);

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
            if let Some(target) =
                script_method_declaration_target(graph, source_id, source.text(), &token)
                && let Some(item) = self.call_hierarchy_item_for_target(&target)
            {
                return vec![item];
            }
        }

        for scope in self.call_scopes() {
            if scope.span.source != source_id || !scope.span.contains(offset) {
                continue;
            }
            if let Some(target) = declaration_call_target(scope.bindings, &token)
                .and_then(|target| self.call_hierarchy_function_target(target))
                && let Some(item) =
                    self.call_hierarchy_item_for_target(&CallHierarchyTarget::Function(target))
            {
                return vec![item];
            }
            let Some(method_name) = token_text(source.text(), token.range) else {
                continue;
            };
            if let Some(target) = script_method_target_for_member(
                graph,
                &facts,
                source.text(),
                source_id,
                scope.bindings,
                method_name,
                token.range,
            )
            .map(CallHierarchyTarget::Method)
                && let Some(item) = self.call_hierarchy_item_for_target(&target)
            {
                return vec![item];
            }
        }

        Vec::new()
    }

    #[must_use]
    pub fn incoming_calls(&self, item: &CallHierarchyItem) -> Vec<IncomingCall> {
        let Some(target) = self.call_hierarchy_target_for_item(item) else {
            return Vec::new();
        };
        let mut calls = Vec::new();

        for scope in self.call_scopes() {
            let ranges = self.call_ranges_to(scope.bindings, scope.span, &target);
            if ranges.is_empty() {
                continue;
            }
            if let Some(from) = self.call_hierarchy_item_for_target(&scope.caller) {
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
        let Some(caller) = self.call_hierarchy_target_for_item(item) else {
            return Vec::new();
        };
        let Some(scope) = self.call_scope_for_target(&caller) else {
            return Vec::new();
        };
        let mut calls = Vec::<OutgoingCall>::new();

        for (target, range) in self.resolved_call_ranges(scope.bindings, scope.span) {
            let Some(to) = self.call_hierarchy_item_for_target(&target) else {
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

    fn call_ranges_to(
        &self,
        bindings: &BindingMap,
        scope_span: Span,
        target: &CallHierarchyTarget,
    ) -> Vec<DiagnosticRange> {
        self.resolved_call_ranges(bindings, scope_span)
            .into_iter()
            .filter_map(|(resolved, range)| (&resolved == target).then_some(range))
            .collect()
    }

    fn resolved_call_ranges(
        &self,
        bindings: &BindingMap,
        scope_span: Span,
    ) -> Vec<(CallHierarchyTarget, DiagnosticRange)> {
        let mut calls = bindings
            .resolutions()
            .filter_map(|(expression, resolution)| match resolution {
                BindingResolution::Declaration(target) => {
                    let expression = bindings.expression(expression)?;
                    let target = self.call_hierarchy_function_target(*target)?;
                    self.call_range_for_expression(expression.span)
                        .map(|range| (CallHierarchyTarget::Function(target), range))
                }
                BindingResolution::Local(_)
                | BindingResolution::Import(_)
                | BindingResolution::QualifiedPath(_) => None,
            })
            .collect::<Vec<_>>();

        calls.extend(self.resolved_method_call_ranges(bindings, scope_span));
        calls
    }

    fn call_range_for_expression(&self, span: Span) -> Option<DiagnosticRange> {
        let source = self.source_record_for_call_hierarchy(span.source)?;
        let range = span_text_range(span)?;
        is_call_callee(source.text(), range).then(|| diagnostic_range(source.text(), range))
    }

    fn call_hierarchy_function_target(&self, target: HirDeclId) -> Option<HirDeclId> {
        let graph = self.hir_db().graph();
        let target = graph.declaration(target)?;
        (target.kind == DeclarationKind::Function).then_some(target.id)
    }

    fn resolved_method_call_ranges(
        &self,
        bindings: &BindingMap,
        scope_span: Span,
    ) -> Vec<(CallHierarchyTarget, DiagnosticRange)> {
        let Some(source) = self.source_record_for_call_hierarchy(scope_span.source) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        member_method_call_ranges(source.source_id(), source.text(), scope_span)
            .into_iter()
            .filter_map(|range| {
                script_method_target_for_member(
                    graph,
                    &facts,
                    source.text(),
                    source.source_id(),
                    bindings,
                    token_text(source.text(), range)?,
                    range,
                )
                .map(|target| {
                    (
                        CallHierarchyTarget::Method(target),
                        diagnostic_range(source.text(), range),
                    )
                })
            })
            .collect()
    }

    fn call_hierarchy_target_for_item(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<CallHierarchyTarget> {
        self.call_hierarchy_declaration_for_item(item)
            .map(CallHierarchyTarget::Function)
            .or_else(|| {
                self.call_hierarchy_method_for_item(item)
                    .map(CallHierarchyTarget::Method)
            })
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

    fn call_hierarchy_method_for_item(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<ScriptMethodCallTarget> {
        let source = self.source_db().records().get(item.document_id())?;
        let graph = self.hir_db().graph();
        graph.declarations().find_map(|declaration| {
            let metadata = graph.impl_metadata(declaration.id)?;
            if declaration.span.source != source.source_id() {
                return None;
            }
            metadata
                .methods
                .iter()
                .find(|method| method.name == item.name())
                .and_then(|method| {
                    let target = ScriptMethodCallTarget {
                        owner: declaration.id,
                        method_node: method.node,
                        method: method.name.clone(),
                    };
                    self.call_hierarchy_item_for_method(&target)
                        .is_some_and(|candidate| candidate.selection_range == item.selection_range)
                        .then_some(target)
                })
        })
    }

    fn call_hierarchy_item_for_target(
        &self,
        target: &CallHierarchyTarget,
    ) -> Option<CallHierarchyItem> {
        match target {
            CallHierarchyTarget::Function(declaration) => {
                let declaration = self.hir_db().graph().declaration(*declaration)?;
                self.call_hierarchy_item_for_declaration(declaration)
            }
            CallHierarchyTarget::Method(method) => self.call_hierarchy_item_for_method(method),
        }
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

    fn call_hierarchy_item_for_method(
        &self,
        target: &ScriptMethodCallTarget,
    ) -> Option<CallHierarchyItem> {
        let graph = self.hir_db().graph();
        let declaration = graph.declaration(target.owner)?;
        let metadata = graph.impl_metadata(target.owner)?;
        let method = metadata
            .methods
            .iter()
            .find(|method| method.node == target.method_node && method.name == target.method)?;
        let source = self.source_record_for_call_hierarchy(declaration.span.source)?;
        let span_range = span_text_range(declaration.span)?;
        let name_range = method_name_range_in_text(source.text(), span_range, &method.name)
            .unwrap_or(span_range);
        Some(CallHierarchyItem::new(
            method.name.clone(),
            source.document_id().clone(),
            diagnostic_range(source.text(), span_range),
            diagnostic_range(source.text(), name_range),
        ))
    }

    fn call_scopes(&self) -> Vec<CallScope<'_>> {
        let graph = self.hir_db().graph();
        let mut scopes = Vec::new();
        for declaration in graph.declarations() {
            if declaration.kind == DeclarationKind::Function
                && let Some(bindings) = graph.bindings(declaration.id)
            {
                scopes.push(CallScope {
                    caller: CallHierarchyTarget::Function(declaration.id),
                    span: declaration.span,
                    bindings,
                });
            }
            if declaration.kind == DeclarationKind::Impl
                && let Some(metadata) = graph.impl_metadata(declaration.id)
            {
                for method in &metadata.methods {
                    if let Some(bindings) = graph.impl_method_bindings(method.node) {
                        scopes.push(CallScope {
                            caller: CallHierarchyTarget::Method(ScriptMethodCallTarget {
                                owner: declaration.id,
                                method_node: method.node,
                                method: method.name.clone(),
                            }),
                            span: method.span,
                            bindings,
                        });
                    }
                }
            }
        }
        scopes
    }

    fn call_scope_for_target(&self, target: &CallHierarchyTarget) -> Option<CallScope<'_>> {
        self.call_scopes()
            .into_iter()
            .find(|scope| &scope.caller == target)
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
enum CallHierarchyTarget {
    Function(HirDeclId),
    Method(ScriptMethodCallTarget),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ScriptMethodCallTarget {
    owner: HirDeclId,
    method_node: HirNodeId,
    method: String,
}

struct CallScope<'a> {
    caller: CallHierarchyTarget,
    span: Span,
    bindings: &'a BindingMap,
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

fn script_method_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &CallHierarchyToken,
) -> Option<CallHierarchyTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Impl
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        let span_range = span_text_range(declaration.span)?;
        for method in &metadata.methods {
            let Some(name_range) = method_name_range_in_text(text, span_range, &method.name) else {
                continue;
            };
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(CallHierarchyTarget::Method(ScriptMethodCallTarget {
                    owner: declaration.id,
                    method_node: method.node,
                    method: method.name.clone(),
                }));
            }
        }
    }
    None
}

fn script_method_target_for_member(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    method: &str,
    member_range: TextRange,
) -> Option<ScriptMethodCallTarget> {
    if !is_call_callee(text, member_range) {
        return None;
    }
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = type_fact_for_resolution(resolution, facts)?;
    script_method_owner(graph, &receiver, method)
}

fn script_method_owner(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
) -> Option<ScriptMethodCallTarget> {
    let owner_names = record_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Impl {
            return None;
        }
        let metadata = graph.impl_metadata(declaration.id)?;
        let matches_owner = owner_names.iter().any(|owner| {
            metadata
                .target_path
                .last()
                .is_some_and(|name| name == owner)
                || metadata.target_path.join("::") == *owner
        });
        if !matches_owner {
            return None;
        }
        metadata
            .methods
            .iter()
            .find(|entry| entry.name == method)
            .map(|entry| ScriptMethodCallTarget {
                owner: declaration.id,
                method_node: entry.node,
                method: entry.name.clone(),
            })
    })
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

fn member_method_call_ranges(source_id: SourceId, text: &str, scope: Span) -> Vec<TextRange> {
    lex(source_id, text)
        .tokens
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(_) => {
                let range = span_text_range(token.span)?;
                let start = u32::try_from(range.start).ok()?;
                (scope.contains(start)
                    && is_call_callee(text, range)
                    && member_receiver_range(text, range.start).is_some())
                .then_some(range)
            }
            TokenKind::Int(_)
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
    let before_member = text.get(..member_start)?;
    let before_dot = before_member.trim_end();
    if !before_dot.ends_with('.') {
        return None;
    }
    let before_receiver = before_dot[..before_dot.len().saturating_sub(1)].trim_end();
    let end = before_receiver.len();
    let start = before_receiver
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
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

    #[test]
    fn call_hierarchy_uses_resolved_script_method_calls() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub struct Reward {
    amount: i64
}

impl Reward {
    pub fn grant(self, amount: i64) -> i64 { return amount }
}

pub fn main(reward: Reward) -> i64 {
    let first = reward.grant(1)
    return reward.grant(first)
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

        let prepared_from_declaration = databases.prepare_call_hierarchy(
            &main,
            Position::new(
                5,
                line(text, 5)
                    .find("grant")
                    .expect("method declaration should exist"),
            ),
        );
        let prepared_from_call = databases.prepare_call_hierarchy(
            &main,
            Position::new(
                9,
                line(text, 9)
                    .find("grant")
                    .expect("method call should exist"),
            ),
        );

        assert_eq!(prepared_from_declaration.len(), 1);
        assert_eq!(prepared_from_declaration[0].name(), "grant");
        assert_eq!(prepared_from_declaration[0].document_id(), &main);
        assert_eq!(prepared_from_call, prepared_from_declaration);

        let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from().name(), "main");
        assert_eq!(incoming[0].from().document_id(), &main);
        assert_eq!(incoming[0].from_ranges().len(), 2);
        assert_range(
            incoming[0].from_ranges(),
            9,
            line(text, 9).find("grant").expect("first method call"),
        );
        assert_range(
            incoming[0].from_ranges(),
            10,
            line(text, 10).find("grant").expect("second method call"),
        );

        let main_item = databases
            .prepare_call_hierarchy(
                &main,
                Position::new(8, line(text, 8).find("main").expect("main declaration")),
            )
            .pop()
            .expect("main should prepare a call hierarchy item");
        let outgoing = databases.outgoing_calls(&main_item);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to().name(), "grant");
        assert_eq!(outgoing[0].to().document_id(), &main);
        assert_eq!(outgoing[0].from_ranges().len(), 2);
        assert_range(
            outgoing[0].from_ranges(),
            9,
            line(text, 9).find("grant").expect("first method call"),
        );
        assert_range(
            outgoing[0].from_ranges(),
            10,
            line(text, 10).find("grant").expect("second method call"),
        );
    }

    #[test]
    fn call_hierarchy_uses_resolved_trait_impl_method_calls() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "\
pub fn clamp(value: i64) -> i64 { return value }

pub trait Rewardable {
    fn grant(self, amount: i64) -> i64;
}

pub struct Player { level: i64 }

impl Rewardable for Player {
    fn grant(self, amount: i64) -> i64 { return clamp(amount) }
}

pub fn main(player: Player) -> i64 {
    let first = player.grant(1)
    return player.grant(first)
}";
        let databases = databases_for(vec![SourceFileSnapshot::new(main.clone(), text)]);

        let prepared_from_declaration = databases.prepare_call_hierarchy(
            &main,
            Position::new(
                9,
                line(text, 9)
                    .find("grant")
                    .expect("trait impl method declaration should exist"),
            ),
        );
        let prepared_from_call = databases.prepare_call_hierarchy(
            &main,
            Position::new(
                13,
                line(text, 13)
                    .find("grant")
                    .expect("trait impl method call should exist"),
            ),
        );

        assert_eq!(prepared_from_declaration.len(), 1);
        assert_eq!(prepared_from_declaration[0].name(), "grant");
        assert_eq!(prepared_from_declaration[0].document_id(), &main);
        assert_eq!(prepared_from_call, prepared_from_declaration);

        let incoming = databases.incoming_calls(&prepared_from_declaration[0]);
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from().name(), "main");
        assert_eq!(incoming[0].from().document_id(), &main);
        assert_eq!(incoming[0].from_ranges().len(), 2);
        assert_range(
            incoming[0].from_ranges(),
            13,
            line(text, 13)
                .find("grant")
                .expect("first trait method call"),
        );
        assert_range(
            incoming[0].from_ranges(),
            14,
            line(text, 14)
                .find("grant")
                .expect("second trait method call"),
        );

        let outgoing = databases.outgoing_calls(&prepared_from_declaration[0]);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to().name(), "clamp");
        assert_eq!(outgoing[0].to().document_id(), &main);
        assert_eq!(outgoing[0].from_ranges().len(), 1);
        assert_range(
            outgoing[0].from_ranges(),
            9,
            line(text, 9).find("clamp").expect("helper call"),
        );
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
