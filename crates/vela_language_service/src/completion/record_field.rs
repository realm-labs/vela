use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::StructFieldHint;
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, FunctionItem, ItemKind, SourceFile, Stmt, StmtKind,
};

use super::{
    CompletionContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    accumulator::CompletionAccumulator, display_type_detail_parts, model::RecordConstructor,
};
use crate::symbol_ref::schema_member_symbol;

pub(super) fn record_constructor_at(
    source: &SourceFile,
    offset: usize,
) -> Option<RecordConstructor> {
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

pub(super) fn record_field_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let Some(constructor) = context.record_constructor.as_ref() else {
        return Vec::new();
    };
    let mut items = script_record_field_completions(graph, constructor);
    items.extend(schema_record_field_completions(schema, constructor));
    let existing_fields = constructor
        .field_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut accumulator = CompletionAccumulator::new(context.replace_range(), context.prefix());
    accumulator.add_many_matching(items, |item| {
        !existing_fields.contains(&item.label())
            && field_label_matches(item.label(), context.prefix())
    });
    accumulator.into_items()
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
    graph.declaration_by_type_path(
        &constructor.path,
        &constructor.current_module,
        DeclarationKind::Struct,
    )
}

fn field_completion_from_hint(graph: &ModuleGraph, field: &StructFieldHint) -> CompletionItem {
    let fact = field
        .type_hint
        .as_ref()
        .map_or(TypeFact::Unknown, |hint| type_fact_from_hint(graph, hint));
    let detail_parts = display_type_detail_parts(fact.display_name());
    CompletionItem {
        label: field.name.clone(),
        kind: CompletionKind::Field,
        detail: detail_parts.render(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
        metadata: Default::default(),
    }
    .with_detail_parts(detail_parts)
}

fn schema_record_field_completions(
    schema: &RegistryFacts,
    constructor: &RecordConstructor,
) -> Vec<CompletionItem> {
    let owner = constructor.path.join("::");
    schema
        .fields_for_owner_or_short_name(&owner)
        .into_iter()
        .map(|field| {
            let owner = field.owner;
            let name = field.name;
            let detail_parts = display_type_detail_parts(field.fact.display_name());
            CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Field,
                detail: detail_parts.render(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: None,
                metadata: Default::default(),
            }
            .with_detail_parts(detail_parts)
            .with_symbol(schema_member_symbol(&owner, &name))
        })
        .collect()
}

fn field_label_matches(label: &str, prefix: &str) -> bool {
    prefix.is_empty() || label.starts_with(prefix)
}
