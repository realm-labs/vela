use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::{HirDeclId, HirNodeId};
use vela_hir::module_graph::{Declaration, DeclarationKind, Import, ImportResolution, ModuleGraph};

use crate::{
    DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position, QueryContext,
    TextRange, member_access, references::schema as reference_schema,
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
        let Some(query) = QueryContext::from_databases(self, document_id, position) else {
            return Vec::new();
        };
        let Some(source) = query.source_record() else {
            return Vec::new();
        };
        let Some(range) = query.identifier_range() else {
            return Vec::new();
        };
        let token = CallHierarchyToken { range };
        let source_id = source.source_id();
        let Ok(offset) = u32::try_from(token.range.start) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();

        if let Some(target) = self.schema_method_declaration_target(source_id, &token)
            && let Some(item) = self.call_hierarchy_item_for_target(&target)
        {
            return vec![item];
        }
        if let Some(target) = imported_function_target(graph, source_id, source.text(), &token)
            && let Some(item) =
                self.call_hierarchy_item_for_target(&CallHierarchyTarget::Function(target))
        {
            return vec![item];
        }

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
            if let Some(target) =
                trait_method_declaration_target(graph, source_id, source.text(), &token)
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
            let target = query
                .member_receiver_range()
                .or_else(|| query.call_member_receiver_range())
                .filter(|_| is_call_callee(source.text(), token.range))
                .and_then(|receiver| query.type_fact_for_range(self, receiver))
                .and_then(|receiver| {
                    method_target_for_receiver_fact(
                        graph,
                        self.schema_db().facts(),
                        &receiver,
                        method_name,
                    )
                });
            if let Some(target) = target
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

        calls.extend(self.resolved_method_call_ranges(scope_span));
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
        scope_span: Span,
    ) -> Vec<(CallHierarchyTarget, DiagnosticRange)> {
        let Some(source) = self.source_record_for_call_hierarchy(scope_span.source) else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let Some(parsed) = self.parse_db().parsed_source(source.document_id()) else {
            return Vec::new();
        };
        member_access::member_call_sites(parsed)
            .into_iter()
            .filter(|site| span_contains_range(scope_span, site.member_range))
            .filter_map(|site| {
                let receiver = crate::query_context::type_fact_for_source_range(
                    self,
                    source.source_id(),
                    site.receiver_range,
                )?;
                method_target_for_receiver_fact(
                    graph,
                    self.schema_db().facts(),
                    &receiver,
                    &site.member,
                )
                .map(|target| (target, diagnostic_range(source.text(), site.member_range)))
            })
            .collect()
    }

    fn call_hierarchy_target_for_item(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<CallHierarchyTarget> {
        self.call_hierarchy_schema_method_for_item(item)
            .map(CallHierarchyTarget::SchemaMethod)
            .or_else(|| {
                self.call_hierarchy_declaration_for_item(item)
                    .map(CallHierarchyTarget::Function)
            })
            .or_else(|| {
                self.call_hierarchy_method_for_item(item)
                    .map(CallHierarchyTarget::Method)
            })
            .or_else(|| {
                self.call_hierarchy_trait_method_for_item(item)
                    .map(CallHierarchyTarget::TraitMethod)
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
            CallHierarchyTarget::TraitMethod(method) => {
                self.call_hierarchy_item_for_trait_method(method)
            }
            CallHierarchyTarget::SchemaMethod(method) => {
                self.call_hierarchy_item_for_schema_method(method)
            }
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

    fn call_hierarchy_trait_method_for_item(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<TraitMethodCallTarget> {
        let source = self.source_db().records().get(item.document_id())?;
        let graph = self.hir_db().graph();
        graph.declarations().find_map(|declaration| {
            if declaration.kind != DeclarationKind::Trait
                || declaration.span.source != source.source_id()
            {
                return None;
            }
            let shape = graph.trait_shape(declaration.id)?;
            shape
                .methods
                .iter()
                .find(|method| method.name == item.name())
                .and_then(|method| {
                    let target = TraitMethodCallTarget {
                        owner: declaration.id,
                        method: method.name.clone(),
                    };
                    self.call_hierarchy_item_for_trait_method(&target)
                        .is_some_and(|candidate| candidate.selection_range == item.selection_range)
                        .then_some(target)
                })
        })
    }

    fn call_hierarchy_item_for_trait_method(
        &self,
        target: &TraitMethodCallTarget,
    ) -> Option<CallHierarchyItem> {
        let graph = self.hir_db().graph();
        let declaration = graph.declaration(target.owner)?;
        let shape = graph.trait_shape(target.owner)?;
        let method = shape
            .methods
            .iter()
            .find(|method| method.name == target.method)?;
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

    fn schema_method_declaration_target(
        &self,
        source_id: SourceId,
        token: &CallHierarchyToken,
    ) -> Option<CallHierarchyTarget> {
        let locations = self.schema_db().source_locations();
        let facts = self.schema_db().facts();
        for method in facts.methods() {
            let Some(span) = locations.method_span(&method.owner, &method.name) else {
                continue;
            };
            if span.source == source_id && span_contains_range(span, token.range) {
                return Some(CallHierarchyTarget::SchemaMethod(
                    reference_schema::SchemaMethodReferenceTarget {
                        owner: method.owner,
                        method: method.name,
                        kind: reference_schema::SchemaMethodReferenceKind::Method,
                    },
                ));
            }
        }
        for method in facts.trait_methods() {
            let Some(span) = locations.trait_method_span(&method.owner, &method.name) else {
                continue;
            };
            if span.source == source_id && span_contains_range(span, token.range) {
                return Some(CallHierarchyTarget::SchemaMethod(
                    reference_schema::SchemaMethodReferenceTarget {
                        owner: method.owner,
                        method: method.name,
                        kind: reference_schema::SchemaMethodReferenceKind::TraitMethod,
                    },
                ));
            }
        }
        None
    }

    fn call_hierarchy_schema_method_for_item(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<reference_schema::SchemaMethodReferenceTarget> {
        let source = self.source_db().records().get(item.document_id())?;
        for method in self.schema_db().facts().methods() {
            let target = reference_schema::SchemaMethodReferenceTarget {
                owner: method.owner,
                method: method.name,
                kind: reference_schema::SchemaMethodReferenceKind::Method,
            };
            let span = self
                .schema_db()
                .source_locations()
                .method_span(&target.owner, &target.method)?;
            if span.source == source.source_id()
                && target.method == item.name()
                && self
                    .call_hierarchy_item_for_schema_method(&target)
                    .is_some_and(|candidate| candidate.selection_range == item.selection_range)
            {
                return Some(target);
            }
        }
        for method in self.schema_db().facts().trait_methods() {
            let target = reference_schema::SchemaMethodReferenceTarget {
                owner: method.owner,
                method: method.name,
                kind: reference_schema::SchemaMethodReferenceKind::TraitMethod,
            };
            let span = self
                .schema_db()
                .source_locations()
                .trait_method_span(&target.owner, &target.method)?;
            if span.source == source.source_id()
                && target.method == item.name()
                && self
                    .call_hierarchy_item_for_schema_method(&target)
                    .is_some_and(|candidate| candidate.selection_range == item.selection_range)
            {
                return Some(target);
            }
        }
        None
    }

    fn call_hierarchy_item_for_schema_method(
        &self,
        target: &reference_schema::SchemaMethodReferenceTarget,
    ) -> Option<CallHierarchyItem> {
        let span = match target.kind {
            reference_schema::SchemaMethodReferenceKind::Method => self
                .schema_db()
                .source_locations()
                .method_span(&target.owner, &target.method),
            reference_schema::SchemaMethodReferenceKind::TraitMethod => self
                .schema_db()
                .source_locations()
                .trait_method_span(&target.owner, &target.method),
        }?;
        let source = self.source_record_for_call_hierarchy(span.source)?;
        let range = span_text_range(span)?;
        Some(CallHierarchyItem::new(
            target.method.clone(),
            source.document_id().clone(),
            diagnostic_range(source.text(), range),
            diagnostic_range(source.text(), range),
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
            if declaration.kind == DeclarationKind::Trait
                && let Some(shape) = graph.trait_shape(declaration.id)
            {
                for method in &shape.methods {
                    if let Some(node) = method.default_body_node
                        && let Some(span) = method.default_body_span
                        && let Some(bindings) = graph.trait_default_method_bindings(node)
                    {
                        scopes.push(CallScope {
                            caller: CallHierarchyTarget::TraitMethod(TraitMethodCallTarget {
                                owner: declaration.id,
                                method: method.name.clone(),
                            }),
                            span,
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
    TraitMethod(TraitMethodCallTarget),
    SchemaMethod(reference_schema::SchemaMethodReferenceTarget),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ScriptMethodCallTarget {
    owner: HirDeclId,
    method_node: HirNodeId,
    method: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct TraitMethodCallTarget {
    owner: HirDeclId,
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

fn imported_function_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &CallHierarchyToken,
) -> Option<HirDeclId> {
    for module in graph.module_ids() {
        let Some(imports) = graph.imports(module) else {
            continue;
        };
        for import in imports {
            if import.span.source != source_id || !import_token_matches(text, import, token) {
                continue;
            }
            let ImportResolution::Declaration(declaration) = import.resolution?;
            if graph
                .declaration(declaration)
                .is_some_and(|declaration| declaration.kind == DeclarationKind::Function)
            {
                return Some(declaration);
            }
        }
    }
    None
}

fn import_token_matches(text: &str, import: &Import, token: &CallHierarchyToken) -> bool {
    import
        .alias
        .as_deref()
        .is_some_and(|alias| import_name_matches(text, import, alias, token))
        || import
            .path
            .last()
            .is_some_and(|name| import_name_matches(text, import, name, token))
}

fn import_name_matches(
    text: &str,
    import: &Import,
    name: &str,
    token: &CallHierarchyToken,
) -> bool {
    span_text_range(import.span)
        .and_then(|range| name_range_in_text(text, range, name))
        .is_some_and(|range| range.start <= token.range.start && token.range.end <= range.end)
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

fn trait_method_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &CallHierarchyToken,
) -> Option<CallHierarchyTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Trait
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.trait_shape(declaration.id)?;
        let span_range = span_text_range(declaration.span)?;
        for method in &shape.methods {
            let Some(name_range) = method_name_range_in_text(text, span_range, &method.name) else {
                continue;
            };
            if name_range.start <= token.range.start && token.range.end <= name_range.end {
                return Some(CallHierarchyTarget::TraitMethod(TraitMethodCallTarget {
                    owner: declaration.id,
                    method: method.name.clone(),
                }));
            }
        }
    }
    None
}

fn method_target_for_receiver_fact(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    receiver: &TypeFact,
    method: &str,
) -> Option<CallHierarchyTarget> {
    script_method_owner(graph, receiver, method)
        .map(CallHierarchyTarget::Method)
        .or_else(|| {
            trait_method_owner(graph, receiver, method).map(CallHierarchyTarget::TraitMethod)
        })
        .or_else(|| {
            reference_schema::schema_method_target_for_receiver_fact(schema, receiver, method)
                .map(CallHierarchyTarget::SchemaMethod)
        })
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

fn trait_method_owner(
    graph: &ModuleGraph,
    receiver: &TypeFact,
    method: &str,
) -> Option<TraitMethodCallTarget> {
    let owner_names = trait_owner_names(receiver);
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Trait {
            return None;
        }
        let matches_owner = owner_names
            .iter()
            .any(|owner| declaration_name_matches(declaration, owner));
        if !matches_owner {
            return None;
        }
        let shape = graph.trait_shape(declaration.id)?;
        shape
            .methods
            .iter()
            .find(|entry| entry.name == method)
            .map(|entry| TraitMethodCallTarget {
                owner: declaration.id,
                method: entry.name.clone(),
            })
    })
}

fn declaration_name_matches(declaration: &Declaration, owner: &str) -> bool {
    declaration.name == owner
        || declaration
            .name
            .rsplit("::")
            .next()
            .is_some_and(|short| short == owner)
}

fn record_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_record_owner_names(receiver, &mut owners);
    owners
}

fn trait_owner_names(receiver: &TypeFact) -> Vec<String> {
    let mut owners = Vec::new();
    collect_trait_owner_names(receiver, &mut owners);
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

fn collect_trait_owner_names(receiver: &TypeFact, owners: &mut Vec<String>) {
    match receiver {
        TypeFact::Trait { name } => {
            push_owner_name(owners, name);
            if let Some(short) = name.rsplit("::").next()
                && short != name
            {
                push_owner_name(owners, short);
            }
        }
        TypeFact::Union(facts) => {
            for fact in facts {
                collect_trait_owner_names(fact, owners);
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
        | TypeFact::Record { .. }
        | TypeFact::Module { .. } => {}
    }
}

fn push_owner_name(owners: &mut Vec<String>, name: &str) {
    if !owners.iter().any(|owner| owner == name) {
        owners.push(name.to_owned());
    }
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

fn span_contains_range(span: Span, range: TextRange) -> bool {
    let Ok(start) = u32::try_from(range.start) else {
        return false;
    };
    let Ok(end) = u32::try_from(range.end) else {
        return false;
    };
    span.start <= start && end <= span.end
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
mod tests;
