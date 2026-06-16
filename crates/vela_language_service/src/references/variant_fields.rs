use vela_common::SourceId;
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{Declaration, DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, IfExpr, ItemKind, MatchArm, Pattern,
    RecordPatternField, SourceFile, Stmt, StmtKind,
};

use crate::LanguageServiceDatabases;

use super::{
    Reference, ReferenceKind, ReferenceToken, declaration_name_matches, diagnostic_range,
    name_range_in_text, record_fields, span_text_range, token_text,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct VariantFieldReferenceTarget {
    owner: HirDeclId,
    variant: String,
    field: String,
}

pub(super) fn script_variant_field_references(
    databases: &LanguageServiceDatabases,
    target: &VariantFieldReferenceTarget,
    include_declaration: bool,
) -> Vec<Reference> {
    let graph = databases.hir_db().graph();
    let mut references = Vec::new();

    if include_declaration
        && let Some(reference) = reference_for_variant_field_declaration(databases, target)
    {
        references.push(reference);
    }

    for source in databases.source_db().records().values() {
        if let Some(parsed) = databases.parse_db().parsed_source(source.document_id()) {
            references.extend(variant_constructor_field_references_for_source(
                graph, parsed, source, target,
            ));
            references.extend(variant_pattern_field_references_for_source(
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

pub(super) fn script_variant_field_declaration_target(
    graph: &ModuleGraph,
    source_id: SourceId,
    text: &str,
    token: &ReferenceToken,
) -> Option<VariantFieldReferenceTarget> {
    let start = u32::try_from(token.range.start).ok()?;
    for declaration in graph.declarations() {
        if declaration.kind != DeclarationKind::Enum
            || declaration.span.source != source_id
            || !declaration.span.contains(start)
        {
            continue;
        }
        let shape = graph.enum_shape(declaration.id)?;
        for variant in &shape.variants {
            let EnumVariantFieldsHint::Record(fields) = &variant.fields else {
                continue;
            };
            for field in fields {
                let span_range = span_text_range(field.span)?;
                let name_range = name_range_in_text(text, span_range, &field.name)?;
                if name_range.start <= token.range.start && token.range.end <= name_range.end {
                    return Some(VariantFieldReferenceTarget {
                        owner: declaration.id,
                        variant: variant.name.clone(),
                        field: field.name.clone(),
                    });
                }
            }
        }
    }
    None
}

pub(super) fn script_variant_field_use_target(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    text: &str,
    token: &ReferenceToken,
) -> Option<VariantFieldReferenceTarget> {
    let field = token_text(text, token.range)?;
    variant_constructor_field_use_target(graph, parsed, text, token, field)
        .or_else(|| variant_pattern_field_use_target(graph, parsed, text, token, field))
}

fn reference_for_variant_field_declaration(
    databases: &LanguageServiceDatabases,
    target: &VariantFieldReferenceTarget,
) -> Option<Reference> {
    let graph = databases.hir_db().graph();
    let variant = graph
        .enum_shape(target.owner)?
        .variants
        .iter()
        .find(|variant| variant.name == target.variant)?;
    let EnumVariantFieldsHint::Record(fields) = &variant.fields else {
        return None;
    };
    let field = fields.iter().find(|field| field.name == target.field)?;
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

fn variant_constructor_field_use_target(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    text: &str,
    token: &ReferenceToken,
    field: &str,
) -> Option<VariantFieldReferenceTarget> {
    let mut target = None;
    record_fields::for_each_record_field(parsed, |path, record_field| {
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
            target = variant_field_target_for_path(graph, path, field);
        }
    });
    target
}

fn variant_pattern_field_use_target(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    text: &str,
    token: &ReferenceToken,
    field: &str,
) -> Option<VariantFieldReferenceTarget> {
    let mut target = None;
    for_each_record_variant_pattern_field(parsed, |path, pattern_field| {
        if target.is_some() || pattern_field.name != field {
            return;
        }
        let Some(span_range) = span_text_range(pattern_field.span) else {
            return;
        };
        let Some(name_range) = name_range_in_text(text, span_range, &pattern_field.name) else {
            return;
        };
        if name_range.start <= token.range.start && token.range.end <= name_range.end {
            target = variant_field_target_for_path(graph, path, field);
        }
    });
    target
}

fn variant_constructor_field_references_for_source(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    source: &crate::SourceRecord,
    target: &VariantFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    record_fields::for_each_record_field(parsed, |path, field| {
        if field.name != target.field {
            return;
        }
        if variant_field_target_for_path(graph, path, &target.field).as_ref() != Some(target) {
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

fn variant_pattern_field_references_for_source(
    graph: &ModuleGraph,
    parsed: &SourceFile,
    source: &crate::SourceRecord,
    target: &VariantFieldReferenceTarget,
) -> Vec<Reference> {
    let mut references = Vec::new();
    let text = source.text();
    for_each_record_variant_pattern_field(parsed, |path, field| {
        if field.name != target.field {
            return;
        }
        if variant_field_target_for_path(graph, path, &target.field).as_ref() != Some(target) {
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
            kind: ReferenceKind::Pattern,
        });
    });
    references
}

fn variant_field_target_for_path(
    graph: &ModuleGraph,
    path: &[String],
    field: &str,
) -> Option<VariantFieldReferenceTarget> {
    let (variant, owner_path) = path.split_last()?;
    let owner = graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Enum
            || !enum_path_matches(graph, declaration, owner_path)
        {
            return None;
        }
        let has_field = graph.enum_shape(declaration.id).is_some_and(|shape| {
            shape.variants.iter().any(|entry| {
                entry.name == *variant
                    && matches!(
                        &entry.fields,
                        EnumVariantFieldsHint::Record(fields)
                            if fields.iter().any(|entry| entry.name == field)
                    )
            })
        });
        has_field.then_some(declaration.id)
    })?;
    Some(VariantFieldReferenceTarget {
        owner,
        variant: variant.clone(),
        field: field.to_owned(),
    })
}

fn enum_path_matches(graph: &ModuleGraph, declaration: &Declaration, path: &[String]) -> bool {
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

fn for_each_record_variant_pattern_field(
    parsed: &SourceFile,
    mut visit: impl FnMut(&[String], &RecordPatternField),
) {
    for item in &parsed.items {
        match &item.kind {
            ItemKind::Function(item) => visit_block_patterns(&item.body, &mut visit),
            ItemKind::Trait(item) => {
                for method in &item.methods {
                    if let Some(body) = &method.default_body {
                        visit_block_patterns(body, &mut visit);
                    }
                }
            }
            ItemKind::Impl(item) => {
                for method in &item.methods {
                    visit_block_patterns(&method.function.body, &mut visit);
                }
            }
            ItemKind::Const(item) => visit_expr_patterns(&item.value, &mut visit),
            ItemKind::Struct(item) => {
                for field in &item.fields {
                    if let Some(value) = &field.default_value {
                        visit_expr_patterns(value, &mut visit);
                    }
                }
            }
            ItemKind::Use(_) | ItemKind::Global(_) | ItemKind::Enum(_) => {}
        }
    }
}

fn visit_block_patterns(block: &Block, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    for statement in &block.statements {
        visit_statement_patterns(statement, visit);
    }
}

fn visit_statement_patterns(
    statement: &Stmt,
    visit: &mut impl FnMut(&[String], &RecordPatternField),
) {
    match &statement.kind {
        StmtKind::Let { value, .. } | StmtKind::Return(value) => {
            if let Some(value) = value {
                visit_expr_patterns(value, visit);
            }
        }
        StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        } => {
            if let Some(index_pattern) = index_pattern {
                visit_pattern_fields(index_pattern, visit);
            }
            visit_pattern_fields(pattern, visit);
            visit_expr_patterns(iterable, visit);
            visit_block_patterns(body, visit);
        }
        StmtKind::Expr(expr) => visit_expr_patterns(expr, visit),
        StmtKind::Block(block) => visit_block_patterns(block, visit),
        StmtKind::Break | StmtKind::Continue => {}
    }
}

fn visit_expr_patterns(expr: &Expr, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    match &expr.kind {
        ExprKind::Match(match_expr) => {
            visit_expr_patterns(&match_expr.scrutinee, visit);
            for arm in &match_expr.arms {
                visit_match_arm_patterns(arm, visit);
            }
        }
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                    visit_expr_patterns(expr, visit);
                }
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => visit_expr_patterns(expr, visit),
        ExprKind::Binary { left, right, .. } => {
            visit_expr_patterns(left, visit);
            visit_expr_patterns(right, visit);
        }
        ExprKind::Assign { target, value, .. } => {
            visit_expr_patterns(target, visit);
            visit_expr_patterns(value, visit);
        }
        ExprKind::Field { base, .. } => visit_expr_patterns(base, visit),
        ExprKind::Call { callee, args } => {
            visit_expr_patterns(callee, visit);
            for argument in args {
                visit_argument_patterns(argument, visit);
            }
        }
        ExprKind::Index { base, index } => {
            visit_expr_patterns(base, visit);
            visit_expr_patterns(index, visit);
        }
        ExprKind::Record { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    visit_expr_patterns(value, visit);
                }
            }
        }
        ExprKind::Array(values) => {
            for value in values {
                visit_expr_patterns(value, visit);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                visit_expr_patterns(&entry.key, visit);
                visit_expr_patterns(&entry.value, visit);
            }
        }
        ExprKind::Lambda { body, .. } => visit_expr_patterns(body, visit),
        ExprKind::If(if_expr) => visit_if_patterns(if_expr, visit),
        ExprKind::Block(block) => visit_block_patterns(block, visit),
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn visit_argument_patterns(
    argument: &Argument,
    visit: &mut impl FnMut(&[String], &RecordPatternField),
) {
    visit_expr_patterns(&argument.value, visit);
}

fn visit_if_patterns(if_expr: &IfExpr, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    visit_expr_patterns(&if_expr.condition, visit);
    visit_block_patterns(&if_expr.then_branch, visit);
    if let Some(else_branch) = &if_expr.else_branch {
        match else_branch {
            ElseBranch::If(if_expr) => visit_if_patterns(if_expr, visit),
            ElseBranch::Block(block) => visit_block_patterns(block, visit),
        }
    }
}

fn visit_match_arm_patterns(
    arm: &MatchArm,
    visit: &mut impl FnMut(&[String], &RecordPatternField),
) {
    visit_pattern_fields(&arm.pattern, visit);
    if let Some(guard) = &arm.guard {
        visit_expr_patterns(guard, visit);
    }
    visit_expr_patterns(&arm.body, visit);
}

fn visit_pattern_fields(pattern: &Pattern, visit: &mut impl FnMut(&[String], &RecordPatternField)) {
    match pattern {
        Pattern::RecordVariant { path, fields } => {
            for field in fields {
                visit(path, field);
                if let Some(pattern) = &field.pattern {
                    visit_pattern_fields(pattern, visit);
                }
            }
        }
        Pattern::TupleVariant { fields, .. } => {
            for field in fields {
                visit_pattern_fields(field, visit);
            }
        }
        Pattern::Binding(_) | Pattern::Path(_) | Pattern::Wildcard | Pattern::Literal(_) => {}
    }
}
