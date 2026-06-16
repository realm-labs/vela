use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::binding::BindingMap;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, ItemKind, MatchArm, RecordField,
    SourceFile, Stmt, StmtKind,
};
use vela_syntax::lexer::lex;
use vela_syntax::token::TokenKind;

use crate::{LanguageServiceDatabases, TextRange};

use super::{
    Reference, ReferenceKind, ReferenceToken, declaration_name_matches, diagnostic_range,
    member_receiver_range, name_range_in_text, record_owner_names, resolved_use_reference_kind,
    span_text_range, token_text, type_fact_for_resolution,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FieldReferenceTarget {
    owner: HirDeclId,
    field: String,
}

pub(super) fn script_field_references(
    databases: &LanguageServiceDatabases,
    target: &FieldReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let facts = AnalysisFacts::from_module_graph(graph);
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_script_field_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        references.extend(script_field_use_references_for_source(
            graph, &facts, source, target,
        ));
        if let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) {
            references.extend(script_record_field_references_for_source(
                graph, parsed, source, target,
            ));
        }
    }

    references.sort_by_key(|reference| {
        let start = reference.range().start();
        (
            reference.document_id().as_str().to_owned(),
            start.line,
            start.character,
            reference.kind(),
        )
    });
    references
}

pub(super) fn script_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
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
                return Some(FieldReferenceTarget {
                    owner: declaration.id,
                    field: field.name.clone(),
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
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    script_field_target_for_member(graph, facts, text, source_id, bindings, field, token.range)
}

pub(super) fn script_record_field_use_target(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    text: &str,
    token: &ReferenceToken,
) -> Option<FieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    let mut target = None;
    for_each_explicit_record_field(parsed, |path, record_field| {
        if target.is_some() || record_field.name != field {
            return;
        }
        let Some(span_range) = span_text_range(record_field.span) else {
            return;
        };
        let Some(name_range) = name_range_in_text(text, span_range, &record_field.name) else {
            return;
        };
        if name_range.start <= token.range.start && token.range.end <= name_range.end {
            target = script_field_target_for_constructor_path(graph, path, field);
        }
    });
    target
}

fn reference_for_script_field_declaration(
    databases: &LanguageServiceDatabases,
    target: &FieldReferenceTarget,
) -> Option<Reference> {
    let graph = databases.hir_db().graph();
    let field = graph
        .struct_shape(target.owner)?
        .fields
        .iter()
        .find(|field| field.name == target.field)?;
    let source = databases
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == field.span.source)?;
    let span_range = span_text_range(field.span)?;
    let name_range =
        name_range_in_text(source.text(), span_range, &field.name).unwrap_or(span_range);
    Some(Reference {
        document_id: source.document_id().clone(),
        range: diagnostic_range(source.text(), name_range),
        kind: ReferenceKind::Declaration,
    })
}

fn script_field_use_references_for_source(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    source: &crate::SourceRecord,
    target: &FieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
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
                facts,
                text,
                source_id,
                bindings,
                &target.field,
                range,
            )
            .as_ref()
                == Some(target)
            {
                references.push(Reference {
                    document_id: source.document_id().clone(),
                    range: diagnostic_range(text, range),
                    kind: resolved_use_reference_kind(text, range),
                });
                break;
            }
        }
    }
    references
}

fn script_record_field_references_for_source(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    source: &crate::SourceRecord,
    target: &FieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for_each_explicit_record_field(parsed, |path, field| {
        if field.name != target.field {
            return;
        }
        if script_field_target_for_constructor_path(graph, path, &target.field).as_ref()
            != Some(target)
        {
            return;
        }
        let Some(span_range) = span_text_range(field.span) else {
            return;
        };
        let Some(name_range) = name_range_in_text(text, span_range, &field.name) else {
            return;
        };
        references.push(Reference {
            document_id: source.document_id().clone(),
            range: diagnostic_range(text, name_range),
            kind: ReferenceKind::Read,
        });
    });
    references
}

fn script_field_target_for_member(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    text: &str,
    source_id: SourceId,
    bindings: &BindingMap,
    field: &str,
    member_range: TextRange,
) -> Option<FieldReferenceTarget> {
    let receiver = member_receiver_range(text, member_range.start)?;
    let start = u32::try_from(receiver.start).ok()?;
    let end = u32::try_from(receiver.end).ok()?;
    let span = Span::new(source_id, start, end);
    let resolution = bindings.resolution_at_span(span)?;
    let receiver = type_fact_for_resolution(resolution, facts)?;
    let owner = script_field_owner(graph, &receiver, field)?;
    Some(FieldReferenceTarget {
        owner,
        field: field.to_owned(),
    })
}

fn script_field_target_for_constructor_path(
    graph: &ModuleGraph,
    path: &[String],
    field: &str,
) -> Option<FieldReferenceTarget> {
    let owner = graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct
            || !constructor_path_matches(graph, declaration, path)
        {
            return None;
        }
        let has_field = graph
            .struct_shape(declaration.id)
            .is_some_and(|shape| shape.fields.iter().any(|entry| entry.name == field));
        has_field.then_some(declaration.id)
    })?;
    Some(FieldReferenceTarget {
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

fn constructor_path_matches(
    graph: &ModuleGraph,
    declaration: &Declaration,
    path: &[String],
) -> bool {
    match path {
        [name] => declaration_name_matches(graph, declaration, name),
        segments => graph
            .module_path(declaration.module)
            .is_some_and(|module_path| {
                module_path
                    .segments()
                    .iter()
                    .chain(std::iter::once(&declaration.name))
                    .eq(segments.iter())
            }),
    }
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

fn for_each_explicit_record_field(
    parsed: &SourceFile,
    mut visit: impl FnMut(&[String], &RecordField),
) {
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Const(item) => visit_expr_record_fields(&item.value, &mut visit),
            ItemKind::Function(item) => visit_block_record_fields(&item.body, &mut visit),
            ItemKind::Struct(item) => {
                for field in &item.fields {
                    if let Some(value) = &field.default_value {
                        visit_expr_record_fields(value, &mut visit);
                    }
                }
            }
            ItemKind::Trait(item) => {
                for method in &item.methods {
                    if let Some(body) = &method.default_body {
                        visit_block_record_fields(body, &mut visit);
                    }
                }
            }
            ItemKind::Impl(item) => {
                for method in &item.methods {
                    visit_block_record_fields(&method.function.body, &mut visit);
                }
            }
            ItemKind::Use(_) | ItemKind::Global(_) | ItemKind::Enum(_) => {}
        }
    }
}

fn visit_block_record_fields(block: &Block, visit: &mut impl FnMut(&[String], &RecordField)) {
    for statement in &block.statements {
        visit_statement_record_fields(statement, visit);
    }
}

fn visit_statement_record_fields(
    statement: &Stmt,
    visit: &mut impl FnMut(&[String], &RecordField),
) {
    match &statement.kind {
        StmtKind::Let { value, .. } | StmtKind::Return(value) => {
            if let Some(value) = value {
                visit_expr_record_fields(value, visit);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            visit_expr_record_fields(iterable, visit);
            visit_block_record_fields(body, visit);
        }
        StmtKind::Expr(expr) => visit_expr_record_fields(expr, visit),
        StmtKind::Block(block) => visit_block_record_fields(block, visit),
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn visit_expr_record_fields(expr: &Expr, visit: &mut impl FnMut(&[String], &RecordField)) {
    match &expr.kind {
        ExprKind::Record { path, fields } => {
            for field in fields {
                if field.value.is_some() {
                    visit(path, field);
                }
                if let Some(value) = &field.value {
                    visit_expr_record_fields(value, visit);
                }
            }
        }
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                    visit_expr_record_fields(expr, visit);
                }
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            visit_expr_record_fields(expr, visit);
        }
        ExprKind::Binary { left, right, .. } => {
            visit_expr_record_fields(left, visit);
            visit_expr_record_fields(right, visit);
        }
        ExprKind::Assign { target, value, .. } => {
            visit_expr_record_fields(target, visit);
            visit_expr_record_fields(value, visit);
        }
        ExprKind::Field { base, .. } => visit_expr_record_fields(base, visit),
        ExprKind::Call { callee, args } => {
            visit_expr_record_fields(callee, visit);
            for argument in args {
                visit_argument_record_fields(argument, visit);
            }
        }
        ExprKind::Index { base, index } => {
            visit_expr_record_fields(base, visit);
            visit_expr_record_fields(index, visit);
        }
        ExprKind::Array(values) => {
            for value in values {
                visit_expr_record_fields(value, visit);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                visit_expr_record_fields(&entry.key, visit);
                visit_expr_record_fields(&entry.value, visit);
            }
        }
        ExprKind::Lambda { body, .. } => visit_expr_record_fields(body, visit),
        ExprKind::If(if_expr) => visit_if_record_fields(if_expr, visit),
        ExprKind::Match(match_expr) => {
            visit_expr_record_fields(&match_expr.scrutinee, visit);
            for arm in &match_expr.arms {
                visit_match_arm_record_fields(arm, visit);
            }
        }
        ExprKind::Block(block) => visit_block_record_fields(block, visit),
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn visit_argument_record_fields(
    argument: &Argument,
    visit: &mut impl FnMut(&[String], &RecordField),
) {
    visit_expr_record_fields(&argument.value, visit);
}

fn visit_if_record_fields(if_expr: &IfExpr, visit: &mut impl FnMut(&[String], &RecordField)) {
    visit_expr_record_fields(&if_expr.condition, visit);
    visit_block_record_fields(&if_expr.then_branch, visit);
    if let Some(else_branch) = &if_expr.else_branch {
        match else_branch {
            ElseBranch::If(if_expr) => visit_if_record_fields(if_expr, visit),
            ElseBranch::Block(block) => visit_block_record_fields(block, visit),
        }
    }
}

fn visit_match_arm_record_fields(arm: &MatchArm, visit: &mut impl FnMut(&[String], &RecordField)) {
    if let Some(guard) = &arm.guard {
        visit_expr_record_fields(guard, visit);
    }
    visit_expr_record_fields(&arm.body, visit);
}
