use vela_syntax::ast::{
    Block, ElseBranch, EnumVariantFields, Expr, ExprKind, FunctionItem, ItemKind, Pattern,
    SourceFile, Stmt, StmtKind, StructField,
};
use vela_syntax::lexer::lex;
use vela_syntax::token::{Keyword, Symbol, Token, TokenKind};

use vela_common::{SourceId, Span};

use crate::{LineIndex, Position, TextRange};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CursorContextKind {
    Item,
    Statement,
    Expression,
    Pattern,
    Type,
    UseImport,
    ModulePath,
    MemberAccess,
    RecordExpressionField,
    RecordTypeField,
    CallArgument,
    LambdaParameter,
    MapKey,
    RenameTarget,
    Unknown,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ModulePathRole {
    Expression,
    Type,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CursorContext {
    kind: CursorContextKind,
    prefix: String,
    replace_range: TextRange,
    module_base: Option<String>,
    module_path_role: ModulePathRole,
    member_receiver: Option<TextRange>,
    call_open: Option<usize>,
    call_callee: Option<TextRange>,
    lambda_method: Option<TextRange>,
}

impl CursorContext {
    #[must_use]
    pub const fn kind(&self) -> CursorContextKind {
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

    #[must_use]
    pub const fn module_path_role(&self) -> ModulePathRole {
        self.module_path_role
    }

    #[must_use]
    pub const fn member_receiver(&self) -> Option<TextRange> {
        self.member_receiver
    }

    #[must_use]
    pub const fn call_open(&self) -> Option<usize> {
        self.call_open
    }

    #[must_use]
    pub const fn call_callee(&self) -> Option<TextRange> {
        self.call_callee
    }

    #[must_use]
    pub const fn lambda_method(&self) -> Option<TextRange> {
        self.lambda_method
    }
}

#[must_use]
pub fn cursor_context_at(
    text: &str,
    position: Position,
    parsed: Option<&SourceFile>,
) -> CursorContext {
    let offset = LineIndex::new(text).offset(position);
    let prefix_start = identifier_prefix_start(text, offset);
    let prefix = text[prefix_start..offset].to_owned();
    let before_prefix = &text[..prefix_start];

    if is_lambda_parameter_context(text, offset) {
        let mut cursor = context(CursorContextKind::LambdaParameter, prefix_start, prefix);
        if let Some(call) = lambda_call_before_pipe(text, offset) {
            cursor.member_receiver = Some(call.receiver);
            cursor.call_open = Some(call.open);
            cursor.lambda_method = Some(call.method);
        }
        return cursor;
    }

    if parsed.is_some_and(|source| is_pattern_context(text, source, prefix_start)) {
        return context(CursorContextKind::Pattern, prefix_start, prefix);
    }

    if parsed.is_some_and(|source| is_record_type_field_context(text, source, prefix_start)) {
        return context(CursorContextKind::RecordTypeField, prefix_start, prefix);
    }

    if is_type_context(text, prefix_start) {
        return context(CursorContextKind::Type, prefix_start, prefix);
    }

    if parsed.is_some_and(|source| is_record_expression_field_context(source, prefix_start)) {
        return context(
            CursorContextKind::RecordExpressionField,
            prefix_start,
            prefix,
        );
    }

    if parsed.is_some_and(|source| is_map_key_context(source, prefix_start)) {
        return context(CursorContextKind::MapKey, prefix_start, prefix);
    }

    if let Some(receiver) =
        parsed.and_then(|source| member_receiver_for_source(source, prefix_start))
    {
        let mut cursor = context(CursorContextKind::MemberAccess, prefix_start, prefix);
        cursor.member_receiver = Some(receiver);
        return cursor;
    }

    if let Some(receiver) = recovered_member_receiver_before_dot(text, prefix_start) {
        let mut cursor = context(CursorContextKind::MemberAccess, prefix_start, prefix);
        cursor.member_receiver = Some(receiver);
        return cursor;
    }

    if let Some(module_path) = module_path_before_colons(text, before_prefix) {
        let mut cursor = context(CursorContextKind::ModulePath, prefix_start, prefix);
        cursor.module_base = Some(module_path.base);
        cursor.module_path_role = module_path.role;
        return cursor;
    }

    if is_use_import_context(text, prefix_start) {
        return context(CursorContextKind::UseImport, prefix_start, prefix);
    }

    if is_item_boundary_context(text, prefix_start, parsed) {
        return context(CursorContextKind::Item, prefix_start, prefix);
    }

    if let Some(callee) = parsed.and_then(|source| call_callee_for_source(source, prefix_start)) {
        let mut cursor = context(CursorContextKind::CallArgument, prefix_start, prefix);
        cursor.call_open = active_call_open(text, offset);
        cursor.call_callee = Some(callee);
        return cursor;
    }

    if let Some(open) = active_call_open(text, offset) {
        let mut cursor = context(CursorContextKind::CallArgument, prefix_start, prefix);
        cursor.call_open = Some(open);
        cursor.call_callee = call_callee_before_open(text, open);
        return cursor;
    }

    if prefix.is_empty() && is_statement_context(parsed, prefix_start) {
        return context(CursorContextKind::Statement, prefix_start, prefix);
    }

    if parsed.is_some_and(|source| offset_is_inside_item(source, prefix_start)) {
        return context(CursorContextKind::Expression, prefix_start, prefix);
    }

    context(CursorContextKind::Unknown, prefix_start, prefix)
}

fn context(kind: CursorContextKind, prefix_start: usize, prefix: String) -> CursorContext {
    CursorContext {
        kind,
        replace_range: TextRange::new(prefix_start, prefix_start + prefix.len()),
        prefix,
        module_base: None,
        module_path_role: ModulePathRole::Expression,
        member_receiver: None,
        call_open: None,
        call_callee: None,
        lambda_method: None,
    }
}

fn is_type_context(text: &str, prefix_start: usize) -> bool {
    let Some(before_prefix) = text.get(..prefix_start) else {
        return false;
    };
    let trimmed = before_prefix.trim_end();
    if trimmed.ends_with("::") {
        return false;
    }
    trimmed.ends_with("->")
        || (trimmed.ends_with(':')
            && !trimmed.ends_with("::")
            && type_annotation_left_side_is_plausible(trimmed))
        || inside_builtin_type_args(trimmed)
}

fn type_annotation_left_side_is_plausible(trimmed: &str) -> bool {
    let before_colon = trimmed.trim_end_matches(':').trim_end();
    let Some(last) = before_colon.char_indices().rev().find_map(|(index, ch)| {
        (!is_identifier_continue(ch)).then_some(&before_colon[index + 1..])
    }) else {
        return false;
    };
    !matches!(last, "case" | "default")
}

fn inside_builtin_type_args(trimmed: &str) -> bool {
    let Some(open) = trimmed.rfind('<') else {
        return false;
    };
    if trimmed[open + 1..].contains('>') {
        return false;
    }
    let before_open = trimmed[..open].trim_end();
    let start = before_open
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    matches!(
        &before_open[start..],
        "Array" | "Set" | "Map" | "Iterator" | "Option" | "Result"
    )
}

fn is_use_import_context(text: &str, prefix_start: usize) -> bool {
    current_line_before(text, prefix_start)
        .trim_start()
        .starts_with("use ")
}

fn is_item_boundary_context(text: &str, prefix_start: usize, parsed: Option<&SourceFile>) -> bool {
    if parsed.is_some_and(|source| offset_is_inside_item(source, prefix_start)) {
        return false;
    }
    let before_prefix = text[..prefix_start].trim_end();
    before_prefix.is_empty()
        || before_prefix.ends_with('}')
        || before_prefix.ends_with(';')
        || current_line_before(text, prefix_start)
            .trim_start()
            .trim_end()
            == "pub"
}

fn is_lambda_parameter_context(text: &str, offset: usize) -> bool {
    let Some(before) = text.get(..offset) else {
        return false;
    };
    let Some(pipe) = before.rfind('|') else {
        return false;
    };
    let Some(open) = active_call_open(before, pipe) else {
        return false;
    };
    if before[open + 1..pipe].contains('|') {
        return false;
    }
    let params = &before[pipe + 1..];
    is_lambda_parameter_prefix(params)
}

fn is_lambda_parameter_prefix(params: &str) -> bool {
    params
        .chars()
        .all(|ch| ch.is_whitespace() || ch == ',' || is_identifier_continue(ch))
}

struct LambdaCallRanges {
    open: usize,
    receiver: TextRange,
    method: TextRange,
}

fn lambda_call_before_pipe(text: &str, offset: usize) -> Option<LambdaCallRanges> {
    let before = text.get(..offset)?;
    let pipe = before.rfind('|')?;
    let open = active_call_open(before, pipe)?;
    member_callee_ranges(before.get(..open)?.trim_end(), open)
}

fn member_callee_ranges(callee: &str, open: usize) -> Option<LambdaCallRanges> {
    let method_end = callee.len();
    let method_start = callee[..method_end]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))?;
    if callee.get(method_start..method_end)?.is_empty() {
        return None;
    }
    let dot = callee[..method_start].trim_end().strip_suffix('.')?;
    let receiver_end = dot.len();
    let receiver_start = dot
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_member_receiver_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (receiver_start < receiver_end).then(|| LambdaCallRanges {
        open,
        receiver: TextRange::new(receiver_start, receiver_end),
        method: TextRange::new(method_start, method_end),
    })
}

fn is_record_expression_field_context(source: &SourceFile, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    source.items.iter().any(|item| match &item.kind {
        ItemKind::Const(item) => record_field_for_expr(&item.value, offset),
        ItemKind::Function(item) => record_field_for_function(item, offset),
        _ => false,
    })
}

fn is_record_type_field_context(text: &str, source: &SourceFile, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    source.items.iter().any(|item| match &item.kind {
        ItemKind::Struct(item) => item
            .fields
            .iter()
            .any(|field| field_name_contains(text, field, offset)),
        ItemKind::Enum(item) => item.variants.iter().any(|variant| match &variant.fields {
            EnumVariantFields::Record(fields) => fields
                .iter()
                .any(|field| field_name_contains(text, field, offset)),
            EnumVariantFields::Unit | EnumVariantFields::Tuple(_) => false,
        }),
        _ => false,
    })
}

fn is_pattern_context(text: &str, source: &SourceFile, offset: usize) -> bool {
    source.items.iter().any(|item| match &item.kind {
        ItemKind::Function(item) => {
            item.params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| pattern_for_expr(text, value, offset))
                || pattern_for_block(text, &item.body, offset)
        }
        ItemKind::Const(item) => pattern_for_expr(text, &item.value, offset),
        _ => false,
    })
}

fn pattern_for_block(text: &str, block: &Block, offset: usize) -> bool {
    block_range(block).is_some_and(|range| {
        range_contains_offset(range, offset)
            && block
                .statements
                .iter()
                .any(|statement| pattern_for_statement(text, statement, offset))
    })
}

fn pattern_for_statement(text: &str, statement: &Stmt, offset: usize) -> bool {
    if !span_contains_usize(statement.span, offset) {
        return false;
    }
    match &statement.kind {
        StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        } => {
            let pattern_region = TextRange::new(
                usize::try_from(statement.span.start).unwrap_or_default(),
                usize::try_from(iterable.span.start).unwrap_or_default(),
            );
            index_pattern.as_ref().is_some_and(|pattern| {
                pattern_contains_offset(text, pattern, pattern_region, offset)
            }) || pattern_contains_offset(text, pattern, pattern_region, offset)
                || pattern_for_expr(text, iterable, offset)
                || pattern_for_block(text, body, offset)
        }
        StmtKind::Let { value, .. } => value
            .as_ref()
            .is_some_and(|value| pattern_for_expr(text, value, offset)),
        StmtKind::Expr(value) | StmtKind::Return(Some(value)) => {
            pattern_for_expr(text, value, offset)
        }
        StmtKind::Block(block) => pattern_for_block(text, block, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => false,
    }
}

fn pattern_for_expr(text: &str, expr: &Expr, offset: usize) -> bool {
    if !span_contains_usize(expr.span, offset) {
        return false;
    }
    match &expr.kind {
        ExprKind::Match(match_expr) => {
            if pattern_for_expr(text, &match_expr.scrutinee, offset) {
                return true;
            }
            let mut arm_start = usize::try_from(match_expr.scrutinee.span.end).unwrap_or_default();
            for arm in &match_expr.arms {
                let arm_end = usize::try_from(arm.body.span.start).unwrap_or_default();
                let arm_region = TextRange::new(arm_start, arm_end);
                if pattern_contains_offset(text, &arm.pattern, arm_region, offset)
                    || arm
                        .guard
                        .as_ref()
                        .is_some_and(|guard| pattern_for_expr(text, guard, offset))
                    || pattern_for_expr(text, &arm.body, offset)
                {
                    return true;
                }
                arm_start = usize::try_from(arm.body.span.end).unwrap_or(arm_start);
            }
            false
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => pattern_for_expr(text, expr, offset),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => pattern_for_expr(text, left, offset) || pattern_for_expr(text, right, offset),
        ExprKind::Field { base, .. } => pattern_for_expr(text, base, offset),
        ExprKind::Call { callee, args } => {
            pattern_for_expr(text, callee, offset)
                || args
                    .iter()
                    .any(|arg| pattern_for_expr(text, &arg.value, offset))
        }
        ExprKind::Index { base, index } => {
            pattern_for_expr(text, base, offset) || pattern_for_expr(text, index, offset)
        }
        ExprKind::Array(values) => values
            .iter()
            .any(|value| pattern_for_expr(text, value, offset)),
        ExprKind::Map(entries) => entries.iter().any(|entry| {
            pattern_for_expr(text, &entry.key, offset)
                || pattern_for_expr(text, &entry.value, offset)
        }),
        ExprKind::Record { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .any(|value| pattern_for_expr(text, value, offset)),
        ExprKind::Lambda { params, body } => {
            params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| pattern_for_expr(text, value, offset))
                || pattern_for_expr(text, body, offset)
        }
        ExprKind::If(if_expr) => {
            pattern_for_expr(text, &if_expr.condition, offset)
                || pattern_for_block(text, &if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| pattern_for_else_branch(text, branch, offset))
        }
        ExprKind::Block(block) => pattern_for_block(text, block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => false,
    }
}

fn pattern_for_else_branch(text: &str, branch: &ElseBranch, offset: usize) -> bool {
    match branch {
        ElseBranch::Block(block) => pattern_for_block(text, block, offset),
        ElseBranch::If(if_expr) => {
            pattern_for_expr(text, &if_expr.condition, offset)
                || pattern_for_block(text, &if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| pattern_for_else_branch(text, branch, offset))
        }
    }
}

fn pattern_contains_offset(
    text: &str,
    pattern: &Pattern,
    search_range: TextRange,
    offset: usize,
) -> bool {
    match pattern {
        Pattern::Binding(name) => ident_occurrence_contains(text, search_range, name, offset),
        Pattern::Path(path) => path_occurrence_contains(text, search_range, path, offset),
        Pattern::TupleVariant { path, fields } => {
            path_occurrence_contains(text, search_range, path, offset)
                || fields
                    .iter()
                    .any(|field| pattern_contains_offset(text, field, search_range, offset))
        }
        Pattern::RecordVariant { path, fields } => {
            path_occurrence_contains(text, search_range, path, offset)
                || fields.iter().any(|field| {
                    span_contains_usize(field.span, offset)
                        || field.pattern.as_ref().is_some_and(|pattern| {
                            let field_start =
                                usize::try_from(field.span.start).unwrap_or(search_range.start);
                            pattern_contains_offset(
                                text,
                                pattern,
                                TextRange::new(field_start, search_range.end),
                                offset,
                            )
                        })
                })
        }
        Pattern::Wildcard | Pattern::Literal(_) => false,
    }
}

fn path_occurrence_contains(
    text: &str,
    search_range: TextRange,
    path: &[String],
    offset: usize,
) -> bool {
    if path.is_empty() {
        return false;
    }
    let joined = path.join("::");
    if ident_occurrence_contains(text, search_range, &joined, offset) {
        return true;
    }
    path.iter()
        .any(|segment| ident_occurrence_contains(text, search_range, segment, offset))
}

fn ident_occurrence_contains(
    text: &str,
    search_range: TextRange,
    ident: &str,
    offset: usize,
) -> bool {
    if ident.is_empty() || !range_contains_offset(search_range, offset) {
        return false;
    }
    let Some(haystack) = text.get(search_range.start..search_range.end) else {
        return false;
    };
    let mut cursor = 0;
    while let Some(relative) = haystack[cursor..].find(ident) {
        let start = search_range.start + cursor + relative;
        let end = start + ident.len();
        if identifier_boundary(text, start, end) && start <= offset && offset <= end {
            return true;
        }
        cursor += relative + ident.len();
    }
    false
}

fn identifier_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn range_contains_offset(range: TextRange, offset: usize) -> bool {
    range.start <= offset && offset <= range.end
}

fn block_range(block: &Block) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(block.span.start).ok()?,
        usize::try_from(block.span.end).ok()?,
    ))
}

fn span_contains_usize(span: Span, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    span.start <= offset && offset <= span.end
}

fn field_name_contains(text: &str, field: &StructField, offset: u32) -> bool {
    let Some(range) = field_name_range(text, field) else {
        return false;
    };
    let Some(offset) = usize::try_from(offset).ok() else {
        return false;
    };
    range.start <= offset && offset <= range.end
}

fn field_name_range(text: &str, field: &StructField) -> Option<TextRange> {
    let start = usize::try_from(field.span.start).ok()?;
    let end = usize::try_from(field.span.end).ok()?;
    let field_text = text.get(start..end)?;
    let name_start = field_text.find(&field.name)?;
    let start = start + name_start;
    Some(TextRange::new(start, start + field.name.len()))
}

fn record_field_for_function(function: &FunctionItem, offset: u32) -> bool {
    function
        .params
        .iter()
        .filter_map(|param| param.default_value.as_ref())
        .any(|value| record_field_for_expr(value, offset))
        || record_field_for_block(&function.body, offset)
}

fn record_field_for_block(block: &Block, offset: u32) -> bool {
    block.span.contains(offset)
        && block
            .statements
            .iter()
            .any(|statement| record_field_for_statement(statement, offset))
}

fn record_field_for_statement(statement: &Stmt, offset: u32) -> bool {
    if !statement.span.contains(offset) {
        return false;
    }
    match &statement.kind {
        StmtKind::Let { value, .. } => value
            .as_ref()
            .is_some_and(|value| record_field_for_expr(value, offset)),
        StmtKind::Expr(value) | StmtKind::Return(Some(value)) => {
            record_field_for_expr(value, offset)
        }
        StmtKind::For { iterable, body, .. } => {
            record_field_for_expr(iterable, offset) || record_field_for_block(body, offset)
        }
        StmtKind::Block(block) => record_field_for_block(block, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => false,
    }
}

fn record_field_for_expr(expr: &Expr, offset: u32) -> bool {
    if !expr.span.contains(offset) {
        return false;
    }
    match &expr.kind {
        ExprKind::Record { .. } => true,
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => record_field_for_expr(expr, offset),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => record_field_for_expr(left, offset) || record_field_for_expr(right, offset),
        ExprKind::Field { base, .. } => record_field_for_expr(base, offset),
        ExprKind::Call { callee, args } => {
            record_field_for_expr(callee, offset)
                || args
                    .iter()
                    .any(|arg| record_field_for_expr(&arg.value, offset))
        }
        ExprKind::Index { base, index } => {
            record_field_for_expr(base, offset) || record_field_for_expr(index, offset)
        }
        ExprKind::Array(values) => values
            .iter()
            .any(|value| record_field_for_expr(value, offset)),
        ExprKind::Map(entries) => entries.iter().any(|entry| {
            record_field_for_expr(&entry.key, offset) || record_field_for_expr(&entry.value, offset)
        }),
        ExprKind::Lambda { params, body } => {
            params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| record_field_for_expr(value, offset))
                || record_field_for_expr(body, offset)
        }
        ExprKind::If(if_expr) => {
            record_field_for_expr(&if_expr.condition, offset)
                || record_field_for_block(&if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| record_field_for_else_branch(branch, offset))
        }
        ExprKind::Match(match_expr) => {
            record_field_for_expr(&match_expr.scrutinee, offset)
                || match_expr
                    .arms
                    .iter()
                    .any(|arm| record_field_for_expr(&arm.body, offset))
        }
        ExprKind::Block(block) => record_field_for_block(block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => false,
    }
}

fn record_field_for_else_branch(branch: &ElseBranch, offset: u32) -> bool {
    match branch {
        ElseBranch::Block(block) => record_field_for_block(block, offset),
        ElseBranch::If(if_expr) => {
            record_field_for_expr(&if_expr.condition, offset)
                || record_field_for_block(&if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| record_field_for_else_branch(branch, offset))
        }
    }
}

fn is_map_key_context(source: &SourceFile, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    source.items.iter().any(|item| match &item.kind {
        ItemKind::Const(item) => map_key_for_expr(&item.value, offset),
        ItemKind::Function(item) => {
            item.params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| map_key_for_expr(value, offset))
                || map_key_for_block(&item.body, offset)
        }
        _ => false,
    })
}

fn member_receiver_for_source(source: &SourceFile, offset: usize) -> Option<TextRange> {
    let offset = u32::try_from(offset).ok()?;
    source.items.iter().find_map(|item| match &item.kind {
        ItemKind::Const(item) => member_receiver_for_expr(&item.value, offset),
        ItemKind::Function(item) => item
            .params
            .iter()
            .filter_map(|param| param.default_value.as_ref())
            .find_map(|value| member_receiver_for_expr(value, offset))
            .or_else(|| member_receiver_for_block(&item.body, offset)),
        _ => None,
    })
}

fn member_receiver_for_block(block: &Block, offset: u32) -> Option<TextRange> {
    block.span.contains(offset).then(|| {
        block
            .statements
            .iter()
            .find_map(|statement| member_receiver_for_statement(statement, offset))
    })?
}

fn member_receiver_for_statement(statement: &Stmt, offset: u32) -> Option<TextRange> {
    if !statement.span.contains(offset) {
        return None;
    }
    match &statement.kind {
        StmtKind::Let { value, .. } => value
            .as_ref()
            .and_then(|value| member_receiver_for_expr(value, offset)),
        StmtKind::Expr(value) | StmtKind::Return(Some(value)) => {
            member_receiver_for_expr(value, offset)
        }
        StmtKind::For { iterable, body, .. } => member_receiver_for_expr(iterable, offset)
            .or_else(|| member_receiver_for_block(body, offset)),
        StmtKind::Block(block) => member_receiver_for_block(block, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => None,
    }
}

fn member_receiver_for_expr(expr: &Expr, offset: u32) -> Option<TextRange> {
    if !expr.span.contains(offset) {
        return None;
    }
    match &expr.kind {
        ExprKind::Field { base, name } => {
            let name_len = u32::try_from(name.len()).ok()?;
            let name_start = expr.span.end.saturating_sub(name_len);
            if (name.is_empty() && offset == expr.span.end)
                || (!name.is_empty() && name_start <= offset && offset <= expr.span.end)
            {
                return span_range(base.span);
            }
            member_receiver_for_expr(base, offset)
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            member_receiver_for_expr(expr, offset)
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => member_receiver_for_expr(left, offset)
            .or_else(|| member_receiver_for_expr(right, offset)),
        ExprKind::Call { callee, args } => member_receiver_for_expr(callee, offset).or_else(|| {
            args.iter()
                .find_map(|arg| member_receiver_for_expr(&arg.value, offset))
        }),
        ExprKind::Index { base, index } => member_receiver_for_expr(base, offset)
            .or_else(|| member_receiver_for_expr(index, offset)),
        ExprKind::Array(values) => values
            .iter()
            .find_map(|value| member_receiver_for_expr(value, offset)),
        ExprKind::Map(entries) => entries.iter().find_map(|entry| {
            member_receiver_for_expr(&entry.key, offset)
                .or_else(|| member_receiver_for_expr(&entry.value, offset))
        }),
        ExprKind::Record { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .find_map(|value| member_receiver_for_expr(value, offset)),
        ExprKind::Lambda { params, body } => params
            .iter()
            .filter_map(|param| param.default_value.as_ref())
            .find_map(|value| member_receiver_for_expr(value, offset))
            .or_else(|| member_receiver_for_expr(body, offset)),
        ExprKind::If(if_expr) => member_receiver_for_expr(&if_expr.condition, offset)
            .or_else(|| member_receiver_for_block(&if_expr.then_branch, offset))
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|branch| member_receiver_for_else_branch(branch, offset))
            }),
        ExprKind::Match(match_expr) => member_receiver_for_expr(&match_expr.scrutinee, offset)
            .or_else(|| {
                match_expr
                    .arms
                    .iter()
                    .find_map(|arm| member_receiver_for_expr(&arm.body, offset))
            }),
        ExprKind::Block(block) => member_receiver_for_block(block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => None,
    }
}

fn member_receiver_for_else_branch(branch: &ElseBranch, offset: u32) -> Option<TextRange> {
    match branch {
        ElseBranch::Block(block) => member_receiver_for_block(block, offset),
        ElseBranch::If(if_expr) => member_receiver_for_expr(&if_expr.condition, offset)
            .or_else(|| member_receiver_for_block(&if_expr.then_branch, offset))
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|branch| member_receiver_for_else_branch(branch, offset))
            }),
    }
}

fn recovered_member_receiver_before_dot(text: &str, offset: usize) -> Option<TextRange> {
    let fragment_start = member_recovery_fragment_start(text, offset);
    let fragment = text.get(fragment_start..offset)?;
    let relative_offset = u32::try_from(offset.checked_sub(fragment_start)?).ok()?;
    let lexed = lex(SourceId::new(0), fragment);
    let tokens: Vec<_> = lexed
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Eof))
        .collect();
    let dot = tokens.iter().position(|token| {
        token.span.end == relative_offset && matches!(token.kind, TokenKind::Symbol(Symbol::Dot))
    })?;
    let start = receiver_start_before_token(&tokens, dot)?;
    Some(TextRange::new(
        fragment_start + usize::try_from(start).ok()?,
        fragment_start + usize::try_from(tokens[dot].span.start).ok()?,
    ))
}

fn member_recovery_fragment_start(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .and_then(|before| {
            before
                .char_indices()
                .rev()
                .find_map(|(index, ch)| matches!(ch, '\n' | ';' | '{').then_some(index + 1))
        })
        .unwrap_or(0)
}

fn receiver_start_before_token(tokens: &[Token], end_index: usize) -> Option<u32> {
    let mut start = primary_start_before_token(tokens, end_index)?;
    while let Some(index) = token_start_index(tokens, start) {
        if index < 2 || !matches!(tokens[index - 1].kind, TokenKind::Symbol(Symbol::Dot)) {
            break;
        }
        start = receiver_start_before_token(tokens, index - 1)?;
    }
    Some(start)
}

fn primary_start_before_token(tokens: &[Token], end_index: usize) -> Option<u32> {
    let last = end_index.checked_sub(1)?;
    match tokens.get(last)? {
        token if is_receiver_leaf_token(token) => Some(token.span.start),
        Token {
            kind: TokenKind::Symbol(Symbol::RParen),
            ..
        } => {
            let open = matching_open_token(tokens, last, Symbol::LParen, Symbol::RParen)?;
            receiver_start_before_token(tokens, open).or_else(|| Some(tokens[open].span.start))
        }
        Token {
            kind: TokenKind::Symbol(Symbol::RBracket),
            ..
        } => {
            let open = matching_open_token(tokens, last, Symbol::LBracket, Symbol::RBracket)?;
            receiver_start_before_token(tokens, open)
        }
        _ => None,
    }
}

fn is_receiver_leaf_token(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::String(_)
            | TokenKind::Bytes(_)
            | TokenKind::Keyword(
                Keyword::SelfValue | Keyword::True | Keyword::False | Keyword::Null
            )
    )
}

fn matching_open_token(
    tokens: &[Token],
    close_index: usize,
    open: Symbol,
    close: Symbol,
) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..=close_index).rev() {
        match tokens[index].kind {
            TokenKind::Symbol(symbol) if symbol == close => depth += 1,
            TokenKind::Symbol(symbol) if symbol == open => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn token_start_index(tokens: &[Token], start: u32) -> Option<usize> {
    tokens.iter().position(|token| token.span.start == start)
}

fn span_range(span: Span) -> Option<TextRange> {
    Some(TextRange::new(
        usize::try_from(span.start).ok()?,
        usize::try_from(span.end).ok()?,
    ))
}

fn call_callee_for_source(source: &SourceFile, offset: usize) -> Option<TextRange> {
    let offset = u32::try_from(offset).ok()?;
    source.items.iter().find_map(|item| match &item.kind {
        ItemKind::Const(item) => call_callee_for_expr(&item.value, offset),
        ItemKind::Function(item) => item
            .params
            .iter()
            .filter_map(|param| param.default_value.as_ref())
            .find_map(|value| call_callee_for_expr(value, offset))
            .or_else(|| call_callee_for_block(&item.body, offset)),
        _ => None,
    })
}

fn call_callee_for_block(block: &Block, offset: u32) -> Option<TextRange> {
    block.span.contains(offset).then(|| {
        block
            .statements
            .iter()
            .find_map(|statement| call_callee_for_statement(statement, offset))
    })?
}

fn call_callee_for_statement(statement: &Stmt, offset: u32) -> Option<TextRange> {
    if !statement.span.contains(offset) {
        return None;
    }
    match &statement.kind {
        StmtKind::Let { value, .. } => value
            .as_ref()
            .and_then(|value| call_callee_for_expr(value, offset)),
        StmtKind::Expr(value) | StmtKind::Return(Some(value)) => {
            call_callee_for_expr(value, offset)
        }
        StmtKind::For { iterable, body, .. } => {
            call_callee_for_expr(iterable, offset).or_else(|| call_callee_for_block(body, offset))
        }
        StmtKind::Block(block) => call_callee_for_block(block, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => None,
    }
}

fn call_callee_for_expr(expr: &Expr, offset: u32) -> Option<TextRange> {
    if !expr.span.contains(offset) {
        return None;
    }
    match &expr.kind {
        ExprKind::Call { callee, args } => call_callee_for_expr(callee, offset)
            .or_else(|| {
                args.iter()
                    .find_map(|arg| call_callee_for_expr(&arg.value, offset))
            })
            .or_else(|| {
                (callee.span.end < offset && offset <= expr.span.end)
                    .then(|| span_range(callee.span))
                    .flatten()
            }),
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => call_callee_for_expr(expr, offset),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => call_callee_for_expr(left, offset).or_else(|| call_callee_for_expr(right, offset)),
        ExprKind::Field { base, .. } => call_callee_for_expr(base, offset),
        ExprKind::Index { base, index } => {
            call_callee_for_expr(base, offset).or_else(|| call_callee_for_expr(index, offset))
        }
        ExprKind::Array(values) => values
            .iter()
            .find_map(|value| call_callee_for_expr(value, offset)),
        ExprKind::Map(entries) => entries.iter().find_map(|entry| {
            call_callee_for_expr(&entry.key, offset)
                .or_else(|| call_callee_for_expr(&entry.value, offset))
        }),
        ExprKind::Record { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .find_map(|value| call_callee_for_expr(value, offset)),
        ExprKind::Lambda { params, body } => params
            .iter()
            .filter_map(|param| param.default_value.as_ref())
            .find_map(|value| call_callee_for_expr(value, offset))
            .or_else(|| call_callee_for_expr(body, offset)),
        ExprKind::If(if_expr) => call_callee_for_expr(&if_expr.condition, offset)
            .or_else(|| call_callee_for_block(&if_expr.then_branch, offset))
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|branch| call_callee_for_else_branch(branch, offset))
            }),
        ExprKind::Match(match_expr) => {
            call_callee_for_expr(&match_expr.scrutinee, offset).or_else(|| {
                match_expr
                    .arms
                    .iter()
                    .find_map(|arm| call_callee_for_expr(&arm.body, offset))
            })
        }
        ExprKind::Block(block) => call_callee_for_block(block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => None,
    }
}

fn call_callee_for_else_branch(branch: &ElseBranch, offset: u32) -> Option<TextRange> {
    match branch {
        ElseBranch::Block(block) => call_callee_for_block(block, offset),
        ElseBranch::If(if_expr) => call_callee_for_expr(&if_expr.condition, offset)
            .or_else(|| call_callee_for_block(&if_expr.then_branch, offset))
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|branch| call_callee_for_else_branch(branch, offset))
            }),
    }
}

fn map_key_for_block(block: &Block, offset: u32) -> bool {
    block.span.contains(offset)
        && block
            .statements
            .iter()
            .any(|statement| map_key_for_statement(statement, offset))
}

fn map_key_for_statement(statement: &Stmt, offset: u32) -> bool {
    if !statement.span.contains(offset) {
        return false;
    }
    match &statement.kind {
        StmtKind::Let { value, .. } => value
            .as_ref()
            .is_some_and(|value| map_key_for_expr(value, offset)),
        StmtKind::Expr(value) | StmtKind::Return(Some(value)) => map_key_for_expr(value, offset),
        StmtKind::For { iterable, body, .. } => {
            map_key_for_expr(iterable, offset) || map_key_for_block(body, offset)
        }
        StmtKind::Block(block) => map_key_for_block(block, offset),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => false,
    }
}

fn map_key_for_expr(expr: &Expr, offset: u32) -> bool {
    if !expr.span.contains(offset) {
        return false;
    }
    match &expr.kind {
        ExprKind::Map(entries) => entries
            .iter()
            .any(|entry| entry.key.span.contains(offset) || map_key_for_expr(&entry.value, offset)),
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => map_key_for_expr(expr, offset),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => map_key_for_expr(left, offset) || map_key_for_expr(right, offset),
        ExprKind::Field { base, .. } => map_key_for_expr(base, offset),
        ExprKind::Call { callee, args } => {
            map_key_for_expr(callee, offset)
                || args.iter().any(|arg| map_key_for_expr(&arg.value, offset))
        }
        ExprKind::Index { base, index } => {
            map_key_for_expr(base, offset) || map_key_for_expr(index, offset)
        }
        ExprKind::Array(values) => values.iter().any(|value| map_key_for_expr(value, offset)),
        ExprKind::Record { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .any(|value| map_key_for_expr(value, offset)),
        ExprKind::Lambda { params, body } => {
            params
                .iter()
                .filter_map(|param| param.default_value.as_ref())
                .any(|value| map_key_for_expr(value, offset))
                || map_key_for_expr(body, offset)
        }
        ExprKind::If(if_expr) => {
            map_key_for_expr(&if_expr.condition, offset)
                || map_key_for_block(&if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| map_key_for_else_branch(branch, offset))
        }
        ExprKind::Match(match_expr) => {
            map_key_for_expr(&match_expr.scrutinee, offset)
                || match_expr
                    .arms
                    .iter()
                    .any(|arm| map_key_for_expr(&arm.body, offset))
        }
        ExprKind::Block(block) => map_key_for_block(block, offset),
        ExprKind::Literal(_)
        | ExprKind::InterpolatedString(_)
        | ExprKind::Path(_)
        | ExprKind::SelfValue
        | ExprKind::Error => false,
    }
}

fn map_key_for_else_branch(branch: &ElseBranch, offset: u32) -> bool {
    match branch {
        ElseBranch::Block(block) => map_key_for_block(block, offset),
        ElseBranch::If(if_expr) => {
            map_key_for_expr(&if_expr.condition, offset)
                || map_key_for_block(&if_expr.then_branch, offset)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| map_key_for_else_branch(branch, offset))
        }
    }
}

fn is_statement_context(parsed: Option<&SourceFile>, prefix_start: usize) -> bool {
    let Some(offset) = u32::try_from(prefix_start).ok() else {
        return false;
    };
    parsed.is_some_and(|source| {
        source.items.iter().any(|item| {
            if !item.span.contains(offset) {
                return false;
            }
            match &item.kind {
                vela_syntax::ast::ItemKind::Function(function) => {
                    function.body.statements.iter().any(|statement| {
                        statement.span.contains(offset) && is_statement_start(statement, offset)
                    })
                }
                _ => false,
            }
        })
    })
}

fn is_statement_start(statement: &Stmt, offset: u32) -> bool {
    match &statement.kind {
        StmtKind::Let { .. } => offset <= statement.span.start.saturating_add(4),
        StmtKind::Return(_) | StmtKind::Break | StmtKind::Continue => true,
        StmtKind::Expr(expr) => offset <= expr.span.start.saturating_add(1),
        StmtKind::For { .. } | StmtKind::Block(_) => {
            offset <= statement.span.start.saturating_add(1)
        }
    }
}

fn active_call_open(text: &str, offset: usize) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in text[..offset].char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn call_callee_before_open(text: &str, open: usize) -> Option<TextRange> {
    let before = text.get(..open)?.trim_end();
    let end = before.len();
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_callee_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| TextRange::new(start, end))
}

fn offset_is_inside_item(source: &SourceFile, offset: usize) -> bool {
    let Some(offset) = u32::try_from(offset).ok() else {
        return false;
    };
    source.items.iter().any(|item| item.span.contains(offset))
}

fn identifier_prefix_start(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_identifier_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0)
}

struct ModulePathContext {
    base: String,
    role: ModulePathRole,
}

fn module_path_before_colons(text: &str, before_prefix: &str) -> Option<ModulePathContext> {
    let before_colons = before_prefix.strip_suffix("::")?;
    let path_start = before_colons
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_module_path_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let module_base = before_colons[path_start..].trim_matches(':');
    (!module_base.is_empty()).then(|| ModulePathContext {
        base: module_base.to_owned(),
        role: if is_type_context(text, path_start) {
            ModulePathRole::Type
        } else {
            ModulePathRole::Expression
        },
    })
}

fn current_line_before(text: &str, offset: usize) -> &str {
    text[..offset].rsplit('\n').next().unwrap_or_default()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_module_path_continue(ch: char) -> bool {
    is_identifier_continue(ch) || ch == ':'
}

fn is_member_receiver_continue(ch: char) -> bool {
    is_identifier_continue(ch) || ch == ':' || ch == '.'
}

fn is_callee_continue(ch: char) -> bool {
    is_identifier_continue(ch) || ch == ':' || ch == '.'
}

#[cfg(test)]
mod tests;
