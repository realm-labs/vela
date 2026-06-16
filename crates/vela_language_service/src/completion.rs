use std::collections::BTreeMap;

use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, CompletionKind as AnalysisCompletionKind,
    declaration_completions, global_completions, member_completions, module_completions,
};
use vela_analysis::facts::AnalysisFacts;
use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{HirTypeHint, StructFieldHint};
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, FunctionItem, ItemKind, SourceFile, Stmt, StmtKind,
};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position, SourceRecord, TextRange};

mod named_argument;

use named_argument::{named_argument_completion_context, script_function_parameter_completions};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompletionKind {
    Binding,
    Const,
    Field,
    Method,
    Module,
    Variant,
    Function,
    Type,
    Trait,
    Parameter,
}

impl From<AnalysisCompletionKind> for CompletionKind {
    fn from(value: AnalysisCompletionKind) -> Self {
        match value {
            AnalysisCompletionKind::Binding => Self::Binding,
            AnalysisCompletionKind::Const => Self::Const,
            AnalysisCompletionKind::Field => Self::Field,
            AnalysisCompletionKind::Method => Self::Method,
            AnalysisCompletionKind::Module => Self::Module,
            AnalysisCompletionKind::Variant => Self::Variant,
            AnalysisCompletionKind::Function => Self::Function,
            AnalysisCompletionKind::Type => Self::Type,
            AnalysisCompletionKind::Trait => Self::Trait,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompletionContextKind {
    Global,
    ModulePath,
    Member,
    RecordField,
    NamedArgument,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionItem {
    label: String,
    kind: CompletionKind,
    detail: String,
    insert_text: Option<String>,
}

impl CompletionItem {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> CompletionKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    #[must_use]
    pub fn insert_text(&self) -> Option<&str> {
        self.insert_text.as_deref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionContext {
    kind: CompletionContextKind,
    prefix: String,
    replace_range: TextRange,
    module_base: Option<String>,
    member_receiver: Option<MemberReceiver>,
    record_constructor: Option<RecordConstructor>,
    call_arguments: Option<CallArgumentContext>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct MemberReceiver {
    range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RecordConstructor {
    path: Vec<String>,
    field_names: Vec<String>,
    current_module: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CallArgumentContext {
    callee: String,
    used_names: Vec<String>,
}

impl CompletionContext {
    #[must_use]
    pub const fn kind(&self) -> CompletionContextKind {
        self.kind
    }

    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    #[must_use]
    pub const fn replace_range(&self) -> TextRange {
        self.replace_range
    }

    #[must_use]
    pub fn module_base(&self) -> Option<&str> {
        self.module_base.as_deref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompletionList {
    context: CompletionContext,
    items: Vec<CompletionItem>,
}

impl CompletionList {
    #[must_use]
    pub fn context(&self) -> &CompletionContext {
        &self.context
    }

    #[must_use]
    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn completion_items(&self, document_id: &DocumentId, position: Position) -> CompletionList {
        let Some(source) = self.source_db().records().get(document_id) else {
            return empty_completion_list(CompletionContext::global(0, ""));
        };
        let context =
            completion_context(source, position, self.parse_db().parsed_source(document_id));
        if matches!(context.kind, CompletionContextKind::Global)
            && let Some(named_context) = named_argument_completion_context(source.text(), position)
        {
            let mut context = context.clone();
            context.kind = CompletionContextKind::NamedArgument;
            context.call_arguments = Some(named_context);
            let items = self.named_argument_completion_items(&context);
            if !items.is_empty() {
                return CompletionList { context, items };
            }
        }
        let items = match context.kind {
            CompletionContextKind::Global => self.global_completion_items(&context),
            CompletionContextKind::ModulePath => self.module_path_completion_items(&context),
            CompletionContextKind::Member => self.member_completion_items(document_id, &context),
            CompletionContextKind::RecordField => self.record_field_completion_items(&context),
            CompletionContextKind::NamedArgument => self.named_argument_completion_items(&context),
        };
        CompletionList { context, items }
    }

    fn global_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let mut items = global_completions(self.schema_db().facts());
        items.extend(declaration_completions(graph, &facts));
        items.extend(module_completions(graph));
        dedupe_and_filter_items(items, |item| {
            label_segment_matches(&item.label, context.prefix())
        })
    }

    fn module_path_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let Some(base) = context.module_base() else {
            return Vec::new();
        };
        let mut items = declaration_completions(graph, &facts);
        items.extend(module_completions(graph));
        dedupe_and_filter_items(items, |item| {
            item.label
                .strip_prefix(base)
                .and_then(|suffix| suffix.strip_prefix("::"))
                .is_some_and(|suffix| suffix.starts_with(context.prefix()))
        })
    }

    fn member_completion_items(
        &self,
        document_id: &DocumentId,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(receiver) = context.member_receiver.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) = self.member_receiver_fact(document_id, receiver) else {
            return Vec::new();
        };
        dedupe_and_filter_items(
            member_completions(self.schema_db().facts(), &receiver_fact),
            |item| label_segment_matches(&item.label, context.prefix()),
        )
    }

    fn member_receiver_fact(
        &self,
        document_id: &DocumentId,
        receiver: &MemberReceiver,
    ) -> Option<TypeFact> {
        let source = self.source_db().records().get(document_id)?;
        let source_id = source.source_id();
        let start = u32::try_from(receiver.range.start).ok()?;
        let end = u32::try_from(receiver.range.end).ok()?;
        let receiver_span = Span::new(source_id, start, end);
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);

        graph.declarations().find_map(|declaration| {
            if declaration.span.source != source_id || !declaration.span.contains(start) {
                return None;
            }
            let bindings = graph.bindings(declaration.id)?;
            let resolution = bindings.resolution_at_span(receiver_span)?;
            type_fact_for_resolution(resolution, bindings, &facts, self.schema_db().facts())
        })
    }

    fn record_field_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let Some(constructor) = context.record_constructor.as_ref() else {
            return Vec::new();
        };
        let graph = self.hir_db().graph();
        let mut items = script_record_field_completions(graph, constructor);
        items.extend(schema_record_field_completions(
            self.schema_db().facts(),
            constructor,
        ));
        let existing_fields = constructor
            .field_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        dedupe_and_filter_service_items(items, |item| {
            !existing_fields.contains(&item.label())
                && label_segment_matches(item.label(), context.prefix())
        })
    }

    fn named_argument_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let Some(call) = context.call_arguments.as_ref() else {
            return Vec::new();
        };
        let used_names = call
            .used_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let items = script_function_parameter_completions(
            self.hir_db().graph(),
            self.schema_db().facts(),
            &call.callee,
            &used_names,
        );
        dedupe_and_filter_service_items(items, |item| {
            label_segment_matches(item.label(), context.prefix())
        })
    }
}

impl CompletionContext {
    fn global(prefix_start: usize, prefix: &str) -> Self {
        Self {
            kind: CompletionContextKind::Global,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
            module_base: None,
            member_receiver: None,
            record_constructor: None,
            call_arguments: None,
        }
    }
}

fn completion_context(
    source: &SourceRecord,
    position: Position,
    parsed: Option<&SourceFile>,
) -> CompletionContext {
    let text = source.text();
    let offset = LineIndex::new(text).offset(position);
    let prefix_start = identifier_prefix_start(text, offset);
    let prefix = &text[prefix_start..offset];
    let before_prefix = &text[..prefix_start];

    if let Some(mut record_constructor) =
        parsed.and_then(|source| record_constructor_at(source, offset))
    {
        record_constructor.current_module = source.module_path().segments().to_vec();
        return CompletionContext {
            kind: CompletionContextKind::RecordField,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver: None,
            record_constructor: Some(record_constructor),
            call_arguments: None,
        };
    }

    if before_prefix.ends_with('.') {
        let member_receiver = member_receiver_before_dot(before_prefix);
        return CompletionContext {
            kind: CompletionContextKind::Member,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: None,
            member_receiver,
            record_constructor: None,
            call_arguments: None,
        };
    }

    if let Some(module_base) = module_base_before_colons(before_prefix) {
        return CompletionContext {
            kind: CompletionContextKind::ModulePath,
            prefix: prefix.to_owned(),
            replace_range: TextRange::new(prefix_start, offset),
            module_base: Some(module_base),
            member_receiver: None,
            record_constructor: None,
            call_arguments: None,
        };
    }

    CompletionContext::global(prefix_start, prefix)
}

fn record_constructor_at(source: &SourceFile, offset: usize) -> Option<RecordConstructor> {
    let offset = u32::try_from(offset).ok()?;
    for item in &source.items {
        match &item.kind {
            ItemKind::Const(item) => {
                if let Some(context) = record_constructor_for_expr(&item.value, offset) {
                    return Some(context);
                }
            }
            ItemKind::Function(item) => {
                if let Some(context) = record_constructor_for_function(item, offset) {
                    return Some(context);
                }
            }
            _ => {}
        }
    }
    None
}

fn record_constructor_for_function(
    function: &FunctionItem,
    offset: u32,
) -> Option<RecordConstructor> {
    for param in &function.params {
        if let Some(value) = param.default_value.as_ref()
            && let Some(context) = record_constructor_for_expr(value, offset)
        {
            return Some(context);
        }
    }
    record_constructor_for_block(&function.body, offset)
}

fn record_constructor_for_block(block: &Block, offset: u32) -> Option<RecordConstructor> {
    if !block.span.contains(offset) {
        return None;
    }
    for statement in &block.statements {
        if let Some(context) = record_constructor_for_statement(statement, offset) {
            return Some(context);
        }
    }
    None
}

fn record_constructor_for_statement(statement: &Stmt, offset: u32) -> Option<RecordConstructor> {
    if !statement.span.contains(offset) {
        return None;
    }
    match &statement.kind {
        StmtKind::Let { value, .. } => value
            .as_ref()
            .and_then(|value| record_constructor_for_expr(value, offset)),
        StmtKind::Expr(value) => record_constructor_for_expr(value, offset),
        StmtKind::Return(Some(value)) => record_constructor_for_expr(value, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => None,
        StmtKind::For { iterable, body, .. } => record_constructor_for_expr(iterable, offset)
            .or_else(|| record_constructor_for_block(body, offset)),
        StmtKind::Block(block) => record_constructor_for_block(block, offset),
    }
}

fn record_constructor_for_expr(expr: &Expr, offset: u32) -> Option<RecordConstructor> {
    if !expr.span.contains(offset) {
        return None;
    }
    match &expr.kind {
        ExprKind::Record { path, fields } => {
            for field in fields {
                if let Some(value) = field.value.as_ref()
                    && let Some(context) = record_constructor_for_expr(value, offset)
                {
                    return Some(context);
                }
            }
            Some(RecordConstructor {
                path: path.clone(),
                field_names: fields.iter().map(|field| field.name.clone()).collect(),
                current_module: Vec::new(),
            })
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            record_constructor_for_expr(expr, offset)
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => record_constructor_for_expr(left, offset)
            .or_else(|| record_constructor_for_expr(right, offset)),
        ExprKind::Field { base, .. } => record_constructor_for_expr(base, offset),
        ExprKind::Call { callee, args } => {
            record_constructor_for_expr(callee, offset).or_else(|| {
                args.iter()
                    .find_map(|arg| record_constructor_for_expr(&arg.value, offset))
            })
        }
        ExprKind::Index { base, index } => record_constructor_for_expr(base, offset)
            .or_else(|| record_constructor_for_expr(index, offset)),
        ExprKind::Array(values) => values
            .iter()
            .find_map(|value| record_constructor_for_expr(value, offset)),
        ExprKind::Map(entries) => entries.iter().find_map(|entry| {
            record_constructor_for_expr(&entry.key, offset)
                .or_else(|| record_constructor_for_expr(&entry.value, offset))
        }),
        ExprKind::Lambda { params, body } => params
            .iter()
            .filter_map(|param| param.default_value.as_ref())
            .find_map(|value| record_constructor_for_expr(value, offset))
            .or_else(|| record_constructor_for_expr(body, offset)),
        ExprKind::If(if_expr) => record_constructor_for_expr(&if_expr.condition, offset)
            .or_else(|| record_constructor_for_block(&if_expr.then_branch, offset))
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|branch| match branch {
                        ElseBranch::Block(block) => record_constructor_for_block(block, offset),
                        ElseBranch::If(if_expr) => record_constructor_for_if(if_expr, offset),
                    })
            }),
        ExprKind::Match(match_expr) => record_constructor_for_expr(&match_expr.scrutinee, offset)
            .or_else(|| {
                match_expr
                    .arms
                    .iter()
                    .find_map(|arm| record_constructor_for_expr(&arm.body, offset))
            }),
        ExprKind::Block(block) => record_constructor_for_block(block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => None,
    }
}

fn record_constructor_for_if(
    if_expr: &vela_syntax::ast::IfExpr,
    offset: u32,
) -> Option<RecordConstructor> {
    if !if_expr.condition.span.contains(offset)
        && !if_expr.then_branch.span.contains(offset)
        && !if_expr
            .else_branch
            .as_ref()
            .is_some_and(|branch| else_branch_contains(branch, offset))
    {
        return None;
    }
    record_constructor_for_expr(&if_expr.condition, offset)
        .or_else(|| record_constructor_for_block(&if_expr.then_branch, offset))
        .or_else(|| {
            if_expr
                .else_branch
                .as_ref()
                .and_then(|branch| match branch {
                    ElseBranch::Block(block) => record_constructor_for_block(block, offset),
                    ElseBranch::If(if_expr) => record_constructor_for_if(if_expr, offset),
                })
        })
}

fn else_branch_contains(branch: &ElseBranch, offset: u32) -> bool {
    match branch {
        ElseBranch::If(if_expr) => {
            if_expr.condition.span.contains(offset)
                || if_expr.then_branch.span.contains(offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| else_branch_contains(branch, offset))
        }
        ElseBranch::Block(block) => block.span.contains(offset),
    }
}

fn identifier_prefix_start(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0)
}

fn module_base_before_colons(before_prefix: &str) -> Option<String> {
    let before_colons = before_prefix.strip_suffix("::")?;
    let start = before_colons
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_module_path_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let module_base = before_colons[start..].trim_matches(':');
    (!module_base.is_empty()).then(|| module_base.to_owned())
}

fn member_receiver_before_dot(before_prefix: &str) -> Option<MemberReceiver> {
    let before_dot = before_prefix.strip_suffix('.')?;
    let end = before_dot.len();
    let start = before_dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| MemberReceiver {
        range: TextRange::new(start, end),
    })
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_module_path_continue(ch: char) -> bool {
    is_identifier_continue(ch) || ch == ':'
}

fn dedupe_and_filter_items(
    items: Vec<AnalysisCompletionItem>,
    matches_context: impl Fn(&AnalysisCompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut deduped = BTreeMap::new();
    for item in items.into_iter().filter(matches_context) {
        deduped
            .entry((item.label.clone(), CompletionKind::from(item.kind)))
            .or_insert_with(|| CompletionItem {
                label: item.label,
                kind: item.kind.into(),
                detail: item.fact.display_name(),
                insert_text: None,
            });
    }
    deduped.into_values().collect()
}

fn dedupe_and_filter_service_items(
    items: Vec<CompletionItem>,
    matches_context: impl Fn(&CompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut deduped = BTreeMap::new();
    for item in items.into_iter().filter(matches_context) {
        deduped
            .entry((item.label.clone(), item.kind))
            .or_insert(item);
    }
    deduped.into_values().collect()
}

fn script_record_field_completions(
    graph: &ModuleGraph,
    constructor: &RecordConstructor,
) -> Vec<CompletionItem> {
    let Some(declaration) = script_record_constructor_declaration(graph, constructor) else {
        return Vec::new();
    };
    let Some(shape) = graph.struct_shape(declaration.id) else {
        return Vec::new();
    };
    shape
        .fields
        .iter()
        .map(|field| field_completion_from_hint(graph, field))
        .collect()
}

fn script_record_constructor_declaration<'a>(
    graph: &'a ModuleGraph,
    constructor: &RecordConstructor,
) -> Option<&'a vela_hir::module_graph::Declaration> {
    let name = constructor.path.last()?;
    graph.declarations().find(|declaration| {
        declaration.kind == DeclarationKind::Struct
            && declaration.name == *name
            && declaration_path_matches(graph, declaration, constructor)
    })
}

fn declaration_path_matches(
    graph: &ModuleGraph,
    declaration: &vela_hir::module_graph::Declaration,
    constructor: &RecordConstructor,
) -> bool {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return false;
    };
    let path = &constructor.path;
    if path.len() == 1 {
        return module_path.segments() == constructor.current_module;
    }
    let expected = path[..path.len().saturating_sub(1)].join("::");
    module_path.join() == expected
}

fn field_completion_from_hint(graph: &ModuleGraph, field: &StructFieldHint) -> CompletionItem {
    let fact = field
        .type_hint
        .as_ref()
        .map_or(TypeFact::Unknown, |hint| type_fact_from_hint(graph, hint));
    CompletionItem {
        label: field.name.clone(),
        kind: CompletionKind::Field,
        detail: fact.display_name(),
        insert_text: None,
    }
}

fn schema_record_field_completions(
    schema: &vela_analysis::registry::RegistryFacts,
    constructor: &RecordConstructor,
) -> Vec<CompletionItem> {
    let owner = constructor.path.join("::");
    let short_owner = constructor.path.last().map(String::as_str);
    schema
        .fields()
        .filter(|field| field.owner == owner || Some(field.owner.as_str()) == short_owner)
        .map(|field| CompletionItem {
            label: field.name,
            kind: CompletionKind::Field,
            detail: field.fact.display_name(),
            insert_text: None,
        })
        .collect()
}

fn completion_type_fact(
    graph: &ModuleGraph,
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> TypeFact {
    let fact = type_fact_from_hint(graph, hint);
    if matches!(fact, TypeFact::Unknown) {
        schema_fact_for_hint(hint, schema).unwrap_or(TypeFact::Unknown)
    } else {
        fact
    }
}

fn label_segment_matches(label: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || label.starts_with(prefix)
        || label
            .rsplit("::")
            .next()
            .is_some_and(|segment| segment.starts_with(prefix))
}

fn type_fact_for_resolution(
    resolution: &BindingResolution,
    bindings: &BindingMap,
    facts: &AnalysisFacts,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    match resolution {
        BindingResolution::Local(local) => {
            let binding = bindings.local(*local)?;
            let fact = facts.local(*local).cloned();
            fact.filter(|fact| !matches!(fact, TypeFact::Unknown))
                .or_else(|| schema_fact_for_local_hint(binding, schema))
        }
        BindingResolution::Declaration(declaration) => facts.declaration(*declaration).cloned(),
        BindingResolution::Import(_) | BindingResolution::QualifiedPath(_) => None,
    }
}

fn schema_fact_for_local_hint(
    binding: &LocalBinding,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    let hint = binding.type_hint.as_ref()?;
    schema_fact_for_hint(hint, schema)
}

fn schema_fact_for_hint(
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    if hint.args.is_empty() {
        let qualified = hint.path.join("::");
        schema
            .type_fact(&qualified)
            .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
            .cloned()
    } else {
        None
    }
}

fn empty_completion_list(context: CompletionContext) -> CompletionList {
    CompletionList {
        context,
        items: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use vela_analysis::registry::RegistryFacts;
    use vela_analysis::type_fact::TypeFact;

    use super::*;
    use crate::{
        SourceFileSnapshot, SourceVersion, Workspace, WorkspaceConfig, WorkspaceRoot,
        assemble_project_sources,
    };

    #[test]
    fn completion_uses_open_overlay_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let files = vec![SourceFileSnapshot::new(
            document.clone(),
            "pub fn disk_only() { return 1 }",
        )];
        let mut workspace = Workspace::new();
        workspace.open_document(
            document.clone(),
            "pub fn overlay_only() { return 2 }",
            SourceVersion::new(2),
        );
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &workspace.snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 7));

        assert_completion(
            &completions,
            "game::main::overlay_only",
            CompletionKind::Function,
        );
        assert_no_completion(&completions, "game::main::disk_only");
    }

    #[test]
    fn global_completion_uses_schema_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let files = vec![SourceFileSnapshot::new(
            document.clone(),
            "pub fn main() { Pla }",
        )];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_function(
            "spawn_player",
            TypeFact::function(vec![TypeFact::STRING], TypeFact::host("Player")),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 18));

        assert_completion(&completions, "Player", CompletionKind::Type);
        assert_no_completion(&completions, "spawn_player");
    }

    #[test]
    fn member_completion_uses_host_schema_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main(player: Player) { player.le }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        schema.insert_method(
            "Player",
            "level_up",
            TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
        );
        databases.set_schema_facts(schema);
        databases.update(&project);

        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find("le }").expect("member prefix") + "le".len()),
        );

        assert_eq!(completions.context().kind(), CompletionContextKind::Member);
        assert_completion(&completions, "level", CompletionKind::Field);
        assert_completion(&completions, "level_up", CompletionKind::Method);
    }

    #[test]
    fn record_field_completion_requires_known_type() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub struct Player { id: String level: i64 }\npub fn main() { let player = Player { id: \"p1\", le } }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(
            &document,
            Position::new(
                1,
                text.lines()
                    .nth(1)
                    .expect("second line")
                    .find("le }")
                    .expect("record prefix")
                    + "le".len(),
            ),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::RecordField
        );
        assert_completion(&completions, "level", CompletionKind::Field);
        assert_no_completion(&completions, "id");

        let unknown_text =
            "pub fn helper() { return 1 }\npub fn main() { let player = Missing { le } }";
        let files = vec![SourceFileSnapshot::new(document.clone(), unknown_text)];
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        databases.update(&project);

        let completions = databases.completion_items(
            &document,
            Position::new(
                1,
                unknown_text
                    .lines()
                    .nth(1)
                    .expect("second line")
                    .find("le }")
                    .expect("unknown prefix")
                    + "le".len(),
            ),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::RecordField
        );
        assert!(completions.items().is_empty(), "{completions:?}");
    }

    #[test]
    fn record_field_completion_uses_schema_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { let player = Player { le } }";
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        schema.insert_field("Player", "name", TypeFact::STRING);
        databases.set_schema_facts(schema);
        databases.update(&project);

        let completions = databases.completion_items(
            &document,
            Position::new(0, text.find("le }").expect("record prefix") + "le".len()),
        );

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::RecordField
        );
        assert_completion(&completions, "level", CompletionKind::Field);
        assert_no_completion(&completions, "name");
    }

    #[test]
    fn module_completion_follows_import_context() {
        let main = DocumentId::from("/workspace/scripts/game/main.vela");
        let reward = DocumentId::from("/workspace/scripts/game/reward.vela");
        let files = vec![
            SourceFileSnapshot::new(main.clone(), "use game::r"),
            SourceFileSnapshot::new(reward, "pub fn grant() { return 1 }"),
        ];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&main, Position::new(0, "use game::r".len()));

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::ModulePath
        );
        assert_eq!(completions.context().module_base(), Some("game"));
        assert_completion(&completions, "game::reward", CompletionKind::Module);
        assert_no_completion(&completions, "game::main");
    }

    #[test]
    fn named_argument_completion_suggests_unused_script_parameters() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
pub fn grant(player: Player, amount: i64, reason: String = "quest") -> bool { return true }
pub fn main(player: Player) { grant(player: player, ) }
"#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(2).expect("main line should exist");
        let position = Position::new(
            2,
            main_line
                .find(", )")
                .expect("call should contain empty argument")
                + ", ".len(),
        );
        let completions = databases.completion_items(&document, position);

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::NamedArgument
        );
        assert_no_completion(&completions, "player");
        assert_completion(&completions, "amount", CompletionKind::Parameter);
        assert_completion(&completions, "reason", CompletionKind::Parameter);
        let amount = completion(&completions, "amount");
        assert_eq!(amount.detail(), "i64");
        assert_eq!(amount.insert_text(), Some("amount: "));
        let reason = completion(&completions, "reason");
        assert_eq!(reason.detail(), "String (defaulted)");
        assert_eq!(reason.insert_text(), Some("reason: "));
    }

    #[test]
    fn named_argument_completion_uses_parameter_prefix() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
pub fn grant(player: Player, amount: i64, reason: String = "quest") -> bool { return true }
pub fn main(player: Player) { grant(player: player, am) }
"#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(2).expect("main line should exist");
        let position = Position::new(
            2,
            main_line.find("am)").expect("call should contain prefix") + "am".len(),
        );
        let completions = databases.completion_items(&document, position);

        assert_eq!(
            completions.context().kind(),
            CompletionContextKind::NamedArgument
        );
        assert_completion(&completions, "amount", CompletionKind::Parameter);
        assert_no_completion(&completions, "reason");
        assert_no_completion(&completions, "player");
    }

    #[test]
    fn member_context_is_detected_without_global_fallback() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let files = vec![SourceFileSnapshot::new(
            document.clone(),
            "pub fn main(player) { player.le }",
        )];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);

        let completions = databases.completion_items(&document, Position::new(0, 31));

        assert_eq!(completions.context().kind(), CompletionContextKind::Member);
        assert!(completions.items().is_empty(), "{completions:?}");
    }

    fn completion<'a>(list: &'a CompletionList, label: &str) -> &'a CompletionItem {
        list.items()
            .iter()
            .find(|item| item.label() == label)
            .unwrap_or_else(|| panic!("completion {label} should exist in {list:?}"))
    }

    fn assert_completion(list: &CompletionList, label: &str, kind: CompletionKind) {
        assert!(
            list.items()
                .iter()
                .any(|item| item.label() == label && item.kind() == kind),
            "{list:?}"
        );
    }

    fn assert_no_completion(list: &CompletionList, label: &str) {
        assert!(
            list.items().iter().all(|item| item.label() != label),
            "{list:?}"
        );
    }
}
