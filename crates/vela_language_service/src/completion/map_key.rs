use vela_common::Span;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    Block, ElseBranch, Expr, ExprKind, FunctionItem, ItemKind, SourceFile, Stmt, StmtKind,
};

use crate::{
    TextRange,
    completion::{
        CompletionInsertFormat, CompletionItem, CompletionKind, dedupe_and_filter_service_items,
        label_segment_matches,
    },
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MapKeyContext {
    pub(super) key_hint: Option<HirTypeHint>,
    pub(super) used_keys: Vec<Vec<String>>,
    pub(super) current_module: Vec<String>,
}

pub(super) fn map_key_at(source: &SourceFile, offset: usize) -> Option<MapKeyContext> {
    let offset = u32::try_from(offset).ok()?;
    for item in &source.items {
        if let ItemKind::Function(item) = &item.kind
            && let Some(context) = map_key_for_function(item, offset)
        {
            return Some(context);
        }
    }
    None
}

pub(super) fn map_key_completion_items(
    graph: &ModuleGraph,
    schema: &vela_analysis::registry::RegistryFacts,
    map_key: &MapKeyContext,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    let Some(key_hint) = map_key.key_hint.as_ref() else {
        return Vec::new();
    };
    let mut items = script_enum_variant_key_completions(graph, map_key, key_hint);
    items.extend(schema_enum_variant_key_completions(schema, key_hint));
    let used_keys = map_key
        .used_keys
        .iter()
        .filter_map(|key| key.last().map(String::as_str))
        .collect::<Vec<_>>();
    dedupe_and_filter_service_items(items, replace_range, prefix, |item| {
        !used_keys.contains(&item.label()) && label_segment_matches(item.label(), prefix)
    })
}

fn map_key_for_function(item: &FunctionItem, offset: u32) -> Option<MapKeyContext> {
    map_key_for_block(&item.body, offset)
}

fn map_key_for_block(block: &Block, offset: u32) -> Option<MapKeyContext> {
    if !block.span.contains(offset) {
        return None;
    }
    block
        .statements
        .iter()
        .find_map(|statement| map_key_for_statement(statement, offset))
}

fn map_key_for_statement(statement: &Stmt, offset: u32) -> Option<MapKeyContext> {
    if !statement.span.contains(offset) {
        return None;
    }
    match &statement.kind {
        StmtKind::Let {
            type_hint,
            value: Some(value),
            ..
        } => map_key_for_expr(value, offset, type_hint.as_ref().and_then(map_key_hint)),
        StmtKind::Return(Some(expr)) | StmtKind::Expr(expr) => map_key_for_expr(expr, offset, None),
        StmtKind::For { iterable, body, .. } => {
            map_key_for_expr(iterable, offset, None).or_else(|| map_key_for_block(body, offset))
        }
        StmtKind::Block(block) => map_key_for_block(block, offset),
        StmtKind::Let { .. } | StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {
            None
        }
    }
}

fn map_key_hint(hint: &vela_syntax::ast::TypeHint) -> Option<HirTypeHint> {
    (hint.path.as_slice() == ["Map"] && hint.args.len() == 2)
        .then(|| HirTypeHint::from_syntax(&hint.args[0]))
}

fn map_key_for_expr(
    expr: &Expr,
    offset: u32,
    expected_key: Option<HirTypeHint>,
) -> Option<MapKeyContext> {
    if !expr.span.contains(offset) {
        return None;
    }
    match &expr.kind {
        ExprKind::Map(entries) => {
            for entry in entries {
                if span_contains_completion_offset(entry.key.span, offset) {
                    return Some(MapKeyContext {
                        key_hint: expected_key.clone(),
                        used_keys: map_entry_path_keys(entries),
                        current_module: Vec::new(),
                    });
                }
                if let Some(context) = map_key_for_expr(&entry.value, offset, None) {
                    return Some(context);
                }
            }
            None
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            map_key_for_expr(expr, offset, expected_key)
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => map_key_for_expr(left, offset, None).or_else(|| map_key_for_expr(right, offset, None)),
        ExprKind::Field { base, .. } => map_key_for_expr(base, offset, None),
        ExprKind::Call { callee, args } => map_key_for_expr(callee, offset, None).or_else(|| {
            args.iter()
                .find_map(|arg| map_key_for_expr(&arg.value, offset, None))
        }),
        ExprKind::Index { base, index } => {
            map_key_for_expr(base, offset, None).or_else(|| map_key_for_expr(index, offset, None))
        }
        ExprKind::Array(values) => values
            .iter()
            .find_map(|value| map_key_for_expr(value, offset, None)),
        ExprKind::Record { fields, .. } => fields.iter().find_map(|field| {
            field
                .value
                .as_ref()
                .and_then(|value| map_key_for_expr(value, offset, None))
        }),
        ExprKind::Lambda { params, body } => params
            .iter()
            .filter_map(|param| param.default_value.as_ref())
            .find_map(|value| map_key_for_expr(value, offset, None))
            .or_else(|| map_key_for_expr(body, offset, None)),
        ExprKind::If(if_expr) => map_key_for_expr(&if_expr.condition, offset, None)
            .or_else(|| map_key_for_block(&if_expr.then_branch, offset))
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|branch| match branch {
                        ElseBranch::Block(block) => map_key_for_block(block, offset),
                        ElseBranch::If(if_expr) => map_key_for_if(if_expr, offset),
                    })
            }),
        ExprKind::Match(match_expr) => map_key_for_expr(&match_expr.scrutinee, offset, None)
            .or_else(|| {
                match_expr
                    .arms
                    .iter()
                    .find_map(|arm| map_key_for_expr(&arm.body, offset, None))
            }),
        ExprKind::Block(block) => map_key_for_block(block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => None,
    }
}

fn map_key_for_if(if_expr: &vela_syntax::ast::IfExpr, offset: u32) -> Option<MapKeyContext> {
    if !if_expr.condition.span.contains(offset)
        && !if_expr.then_branch.span.contains(offset)
        && !if_expr
            .else_branch
            .as_ref()
            .is_some_and(|branch| else_branch_contains(branch, offset))
    {
        return None;
    }
    map_key_for_expr(&if_expr.condition, offset, None)
        .or_else(|| map_key_for_block(&if_expr.then_branch, offset))
        .or_else(|| {
            if_expr
                .else_branch
                .as_ref()
                .and_then(|branch| match branch {
                    ElseBranch::Block(block) => map_key_for_block(block, offset),
                    ElseBranch::If(if_expr) => map_key_for_if(if_expr, offset),
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

fn map_entry_path_keys(entries: &[vela_syntax::ast::MapEntry]) -> Vec<Vec<String>> {
    entries
        .iter()
        .filter_map(|entry| match &entry.key.kind {
            ExprKind::Path(path) => Some(path.clone()),
            _ => None,
        })
        .collect()
}

fn span_contains_completion_offset(span: Span, offset: u32) -> bool {
    span.start <= offset && offset <= span.end
}

fn script_enum_variant_key_completions(
    graph: &ModuleGraph,
    map_key: &MapKeyContext,
    key_hint: &HirTypeHint,
) -> Vec<CompletionItem> {
    let Some(declaration) = script_enum_key_declaration(graph, map_key, key_hint) else {
        return Vec::new();
    };
    let Some(shape) = graph.enum_shape(declaration.id) else {
        return Vec::new();
    };
    shape
        .variants
        .iter()
        .map(|variant| CompletionItem {
            label: variant.name.clone(),
            kind: CompletionKind::Variant,
            detail: key_hint.display(),
            insert_text: None,
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: None,
            metadata: Default::default(),
        })
        .collect()
}

fn script_enum_key_declaration<'a>(
    graph: &'a ModuleGraph,
    map_key: &MapKeyContext,
    key_hint: &HirTypeHint,
) -> Option<&'a vela_hir::module_graph::Declaration> {
    let name = key_hint.path.last()?;
    graph.declarations().find(|declaration| {
        declaration.kind == DeclarationKind::Enum
            && declaration.name == *name
            && type_hint_path_matches(graph, declaration, key_hint, &map_key.current_module)
    })
}

fn schema_enum_variant_key_completions(
    schema: &vela_analysis::registry::RegistryFacts,
    key_hint: &HirTypeHint,
) -> Vec<CompletionItem> {
    let owner = key_hint.path.join("::");
    let short_owner = key_hint.path.last().map(String::as_str);
    schema
        .variants()
        .filter(|variant| variant.owner == owner || Some(variant.owner.as_str()) == short_owner)
        .map(|variant| CompletionItem {
            label: variant.name,
            kind: CompletionKind::Variant,
            detail: key_hint.display(),
            insert_text: None,
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: None,
            metadata: Default::default(),
        })
        .collect()
}

fn type_hint_path_matches(
    graph: &ModuleGraph,
    declaration: &vela_hir::module_graph::Declaration,
    hint: &HirTypeHint,
    current_module: &[String],
) -> bool {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return false;
    };
    let path = &hint.path;
    if path.len() == 1 {
        return module_path.segments() == current_module;
    }
    let expected = path[..path.len().saturating_sub(1)].join("::");
    module_path.join() == expected
}
