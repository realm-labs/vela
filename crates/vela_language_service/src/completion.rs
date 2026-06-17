use vela_analysis::completion::{
    CompletionItem as AnalysisCompletionItem, declaration_completions, global_completions,
    member_completions, module_completions,
};
use vela_analysis::facts::AnalysisFacts;
use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution, LocalBinding, LocalBindingKind};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{HirTypeHint, StructFieldHint};
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, FunctionItem, ItemKind, SourceFile, Stmt, StmtKind,
};

use crate::QueryContext;
use crate::{DocumentId, LanguageServiceDatabases, Position, TextRange};

mod accumulator;
mod context;
mod item;
mod lambda_parameter;
mod map_key;
mod model;
mod module_path;
mod named_argument;
mod pattern;
mod statement;
mod type_hint;

pub use model::{
    CompletionContext, CompletionContextKind, CompletionInsertFormat, CompletionItem,
    CompletionItemMetadata, CompletionKind, CompletionLabelDetails, CompletionList,
    CompletionRelevance, CompletionResolvePayload, CompletionSymbol, CompletionTextEdit,
};

use context::completion_context;
use item::item_keyword_completions;
use lambda_parameter::lambda_parameter_completion_items;
use map_key::map_key_completion_items as map_key_context_completion_items;
use model::{CallArgumentContext, MemberReceiver, RecordConstructor};
use module_path::module_path_completion_items as module_path_context_completion_items;
use named_argument::script_function_parameter_completions;
use pattern::pattern_completion_items as pattern_context_completion_items;
use statement::statement_keyword_completions;
use type_hint::type_hint_completion_items;

use accumulator::CompletionAccumulator;

impl LanguageServiceDatabases {
    #[must_use]
    pub fn completion_items(&self, document_id: &DocumentId, position: Position) -> CompletionList {
        let Some(query) = QueryContext::from_databases(self, document_id, position) else {
            return empty_completion_list(CompletionContext::global(0, ""));
        };
        let context = completion_context(&query);
        let items = match context.kind {
            CompletionContextKind::Global => self.global_completion_items(&query, &context),
            CompletionContextKind::Expression => self.expression_completion_items(&query, &context),
            CompletionContextKind::Item => self.item_completion_items(&context),
            CompletionContextKind::Statement => self.statement_completion_items(&query, &context),
            CompletionContextKind::ModulePath => self.module_path_completion_items(&context),
            CompletionContextKind::Member => self.member_completion_items(document_id, &context),
            CompletionContextKind::RecordField => self.record_field_completion_items(&context),
            CompletionContextKind::MapKey => self.map_key_completion_items(&context),
            CompletionContextKind::Pattern => self.pattern_completion_items(&query, &context),
            CompletionContextKind::NamedArgument => self.named_argument_completion_items(&context),
            CompletionContextKind::LambdaParameter => {
                self.lambda_parameter_completion_items(document_id, &context)
            }
            CompletionContextKind::TypeHint => self.type_hint_completion_items(&context),
        };
        CompletionList { context, items }
    }

    fn global_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let current_module = query
            .module_path()
            .map(|module| module.join())
            .unwrap_or_default();
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let mut items = self.local_completion_items(query, context);
        items.extend(dedupe_and_filter_items(
            global_completions(self.schema_db().facts()),
            context.replace_range(),
            context.prefix(),
            Some(self.schema_db().facts()),
            |item| label_segment_matches(&item.label, context.prefix()),
        ));
        items.extend(dedupe_and_filter_items(
            relative_current_module_items(
                declaration_completions(graph, &facts),
                current_module.as_str(),
            ),
            context.replace_range(),
            context.prefix(),
            None,
            |item| label_segment_matches(&item.label, context.prefix()),
        ));
        items.extend(dedupe_and_filter_items(
            module_completions(graph),
            context.replace_range(),
            context.prefix(),
            None,
            |item| label_segment_matches(&item.label, context.prefix()),
        ));
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
            label_segment_matches(&item.label, context.prefix())
        })
    }

    fn expression_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        self.global_completion_items(query, context)
    }

    fn item_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        dedupe_and_filter_service_items(
            item_keyword_completions(context.prefix()),
            context.replace_range(),
            context.prefix(),
            |item| label_segment_matches(item.label(), context.prefix()),
        )
    }

    fn statement_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let mut items = statement_keyword_completions(context.prefix());
        items.extend(self.global_completion_items(query, context));
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
            label_segment_matches(item.label(), context.prefix())
        })
    }

    fn module_path_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        module_path_context_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            context,
        )
    }

    fn local_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let items = query
            .local_bindings_before_cursor()
            .filter(|local| local.name.starts_with(context.prefix()))
            .map(|local| {
                let kind = match local.kind {
                    LocalBindingKind::Parameter => CompletionKind::Parameter,
                    LocalBindingKind::Let
                    | LocalBindingKind::For
                    | LocalBindingKind::LambdaParameter
                    | LocalBindingKind::Pattern => CompletionKind::Binding,
                };
                let fact = facts.local(local.id).cloned().unwrap_or(TypeFact::Unknown);
                CompletionItem {
                    sort_text: Some(local_sort_text(kind, &local.name)),
                    metadata: Default::default(),
                    label: local.name.clone(),
                    kind,
                    detail: fact.display_name(),
                    insert_text: None,
                    insert_format: CompletionInsertFormat::PlainText,
                }
            })
            .collect::<Vec<_>>();
        let mut accumulator = CompletionAccumulator::new(context.replace_range(), context.prefix());
        accumulator.add_many(items);
        accumulator.into_items()
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
        let schema = self.schema_db().facts();
        let owner = schema_completion_owner(&receiver_fact);
        let items = member_completions(schema, &receiver_fact)
            .into_iter()
            .filter(|item| label_segment_matches(&item.label, context.prefix()))
            .map(|item| {
                let completion = service_item_from_analysis_completion(item, context.prefix());
                owner.as_deref().map_or(completion.clone(), |owner| {
                    enrich_schema_member_completion_item(completion, schema, owner)
                })
            })
            .collect::<Vec<_>>();
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |_| true)
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
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
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
        dedupe_and_filter_service_items(items, context.replace_range(), context.prefix(), |item| {
            label_segment_matches(item.label(), context.prefix())
        })
    }

    fn map_key_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let Some(map_key) = context.map_key.as_ref() else {
            return Vec::new();
        };
        map_key_context_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            map_key,
            context.replace_range(),
            context.prefix(),
        )
    }

    fn pattern_completion_items(
        &self,
        query: &QueryContext<'_>,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let current_module = query
            .module_path()
            .map(|module| module.segments().to_vec())
            .unwrap_or_default();
        let graph = self.hir_db().graph();
        pattern_context_completion_items(
            graph,
            self.schema_db().facts(),
            &current_module,
            context.replace_range(),
            context.prefix(),
        )
    }

    fn lambda_parameter_completion_items(
        &self,
        document_id: &DocumentId,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let Some(lambda_parameter) = context.lambda_parameter.as_ref() else {
            return Vec::new();
        };
        let Some(receiver_fact) =
            self.member_receiver_fact(document_id, &lambda_parameter.receiver)
        else {
            return Vec::new();
        };
        lambda_parameter_completion_items(&receiver_fact, lambda_parameter, context.prefix())
    }

    fn type_hint_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        type_hint_completion_items(
            self.hir_db().graph(),
            self.schema_db().facts(),
            context.replace_range(),
            context.prefix(),
            context.module_base(),
        )
    }
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

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn dedupe_and_filter_items(
    items: Vec<AnalysisCompletionItem>,
    replace_range: TextRange,
    prefix: &str,
    schema: Option<&RegistryFacts>,
    matches_context: impl Fn(&AnalysisCompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    for item in items.into_iter().filter(matches_context) {
        let completion = service_item_from_analysis_completion(item, prefix);
        accumulator.add(enrich_analysis_completion_item(completion, schema));
    }
    accumulator.into_items()
}

fn service_item_from_analysis_completion(
    item: AnalysisCompletionItem,
    prefix: &str,
) -> CompletionItem {
    let kind = item.kind.into();
    let insert_text = callable_insert_text(kind, &item.label);
    let insert_format = completion_insert_format(insert_text.as_ref());
    CompletionItem {
        sort_text: Some(completion_sort_text(kind, &item.label, prefix)),
        metadata: Default::default(),
        label: item.label,
        kind,
        detail: item.fact.display_name(),
        insert_text,
        insert_format,
    }
}

fn enrich_analysis_completion_item(
    item: CompletionItem,
    schema: Option<&RegistryFacts>,
) -> CompletionItem {
    let Some(schema) = schema else {
        return item;
    };
    let label = item.label().to_owned();
    match item.kind() {
        CompletionKind::Type if schema.type_fact(&label).is_some() => item
            .with_documentation(schema.type_docs(&label))
            .with_symbol(CompletionSymbol::Schema(label)),
        CompletionKind::Trait if schema.trait_fact(&label).is_some() => item
            .with_documentation(schema.trait_docs(&label))
            .with_symbol(CompletionSymbol::Schema(label)),
        CompletionKind::Function if schema.function_fact(&label).is_some() => item
            .with_documentation(schema.function_docs(&label))
            .with_symbol(CompletionSymbol::Schema(label)),
        _ => item,
    }
}

fn enrich_schema_member_completion_item(
    item: CompletionItem,
    schema: &RegistryFacts,
    owner: &str,
) -> CompletionItem {
    let label = item.label().to_owned();
    match item.kind() {
        CompletionKind::Field if schema.field_fact(owner, &label).is_some() => item
            .with_documentation(schema.field_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}.{label}"))),
        CompletionKind::Method if schema.method_fact(owner, &label).is_some() => item
            .with_documentation(schema.method_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}.{label}"))),
        CompletionKind::Method if schema.trait_method_fact(owner, &label).is_some() => item
            .with_documentation(schema.trait_method_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}.{label}"))),
        CompletionKind::Variant if schema.variant_fact(owner, &label).is_some() => item
            .with_documentation(schema.variant_docs(owner, &label))
            .with_symbol(CompletionSymbol::Schema(format!("{owner}::{label}"))),
        _ => item,
    }
}

fn schema_completion_owner(fact: &TypeFact) -> Option<String> {
    match fact {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Trait { name } => {
            Some(name.clone())
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(name.clone()),
        _ => None,
    }
}

fn callable_insert_text(kind: CompletionKind, label: &str) -> Option<String> {
    matches!(kind, CompletionKind::Function | CompletionKind::Method)
        .then(|| format!("{label}($0)"))
}

fn completion_insert_format(insert_text: Option<&String>) -> CompletionInsertFormat {
    if insert_text.is_some_and(|text| text.contains("$0")) {
        CompletionInsertFormat::Snippet
    } else {
        CompletionInsertFormat::PlainText
    }
}

fn relative_current_module_items(
    items: Vec<AnalysisCompletionItem>,
    current_module: &str,
) -> Vec<AnalysisCompletionItem> {
    if current_module.is_empty() {
        return items;
    }
    let prefix = format!("{current_module}::");
    items
        .into_iter()
        .map(|mut item| {
            if let Some(relative_label) = item
                .label
                .strip_prefix(&prefix)
                .filter(|relative| !relative.contains("::"))
            {
                item.label = relative_label.to_owned();
            }
            item
        })
        .collect()
}

fn dedupe_and_filter_service_items(
    items: Vec<CompletionItem>,
    replace_range: TextRange,
    prefix: &str,
    matches_context: impl Fn(&CompletionItem) -> bool,
) -> Vec<CompletionItem> {
    let mut accumulator = CompletionAccumulator::new(replace_range, prefix);
    accumulator.add_many_matching(items, matches_context);
    accumulator.into_items()
}

fn completion_sort_text(kind: CompletionKind, label: &str, prefix: &str) -> String {
    format!(
        "{:04}_{:02}_{}",
        completion_kind_rank(kind),
        completion_match_rank(label, prefix),
        label
    )
}

fn local_sort_text(kind: CompletionKind, label: &str) -> String {
    let rank = match kind {
        CompletionKind::Parameter => 0,
        CompletionKind::Keyword => 0,
        CompletionKind::Binding => 1,
        _ => 2,
    };
    format!("{rank:04}_00_{label}")
}

fn completion_kind_rank(kind: CompletionKind) -> u16 {
    match kind {
        CompletionKind::Parameter => 0,
        CompletionKind::Keyword => 0,
        CompletionKind::Binding => 1,
        CompletionKind::Const => 10,
        CompletionKind::Module => 20,
        CompletionKind::Type | CompletionKind::Trait => 30,
        CompletionKind::Function | CompletionKind::Method => 40,
        CompletionKind::Field => 50,
        CompletionKind::Variant => 60,
    }
}

fn completion_match_rank(label: &str, prefix: &str) -> u8 {
    if prefix.is_empty() || label.starts_with(prefix) {
        return 0;
    }
    if label
        .rsplit("::")
        .next()
        .is_some_and(|segment| segment.starts_with(prefix))
    {
        return 1;
    }
    2
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
            && record_constructor_path_matches(graph, declaration, constructor)
    })
}

fn record_constructor_path_matches(
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
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
        metadata: Default::default(),
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
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: None,
            metadata: Default::default(),
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
            .or_else(|| schema.trait_fact(&qualified))
            .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
            .or_else(|| hint.path.last().and_then(|name| schema.trait_fact(name)))
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
mod tests;
