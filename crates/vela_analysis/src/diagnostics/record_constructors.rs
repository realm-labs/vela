use std::collections::BTreeSet;

use vela_common::Diagnostic;
use vela_hir::{
    ids::ModuleId,
    module_graph::{Declaration, DeclarationKind, ModuleGraph},
    type_hint::StructShape,
};
use vela_syntax::ast::{Block, ElseBranch, Expr, ExprKind, RecordField, Stmt, StmtKind};

pub fn record_constructor_diagnostics(
    expr: &Expr,
    graph: &ModuleGraph,
    module: ModuleId,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_record_constructor_diagnostics(expr, graph, module, &mut diagnostics);
    diagnostics
}

fn collect_record_constructor_diagnostics(
    expr: &Expr,
    graph: &ModuleGraph,
    module: ModuleId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
            collect_record_constructor_diagnostics(expr, graph, module, diagnostics);
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
            ..
        } => {
            collect_record_constructor_diagnostics(left, graph, module, diagnostics);
            collect_record_constructor_diagnostics(right, graph, module, diagnostics);
        }
        ExprKind::Field { base, .. } => {
            collect_record_constructor_diagnostics(base, graph, module, diagnostics);
        }
        ExprKind::Call { callee, args } => {
            collect_record_constructor_diagnostics(callee, graph, module, diagnostics);
            for arg in args {
                collect_record_constructor_diagnostics(&arg.value, graph, module, diagnostics);
            }
        }
        ExprKind::Index { base, index } => {
            collect_record_constructor_diagnostics(base, graph, module, diagnostics);
            collect_record_constructor_diagnostics(index, graph, module, diagnostics);
        }
        ExprKind::Array(values) => {
            for value in values {
                collect_record_constructor_diagnostics(value, graph, module, diagnostics);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                collect_record_constructor_diagnostics(&entry.key, graph, module, diagnostics);
                collect_record_constructor_diagnostics(&entry.value, graph, module, diagnostics);
            }
        }
        ExprKind::Record { path, fields } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_record_constructor_diagnostics(value, graph, module, diagnostics);
                }
            }
            diagnose_record_constructor(expr, path, fields, graph, module, diagnostics);
        }
        ExprKind::InterpolatedString(parts) => {
            for part in parts {
                if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                    collect_record_constructor_diagnostics(expr, graph, module, diagnostics);
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_record_constructor_diagnostics(body, graph, module, diagnostics);
        }
        ExprKind::If(if_expr) => {
            collect_record_constructor_diagnostics(&if_expr.condition, graph, module, diagnostics);
            collect_block_diagnostics(&if_expr.then_branch, graph, module, diagnostics);
            if let Some(branch) = &if_expr.else_branch {
                collect_else_branch_diagnostics(branch, graph, module, diagnostics);
            }
        }
        ExprKind::Match(match_expr) => {
            collect_record_constructor_diagnostics(
                &match_expr.scrutinee,
                graph,
                module,
                diagnostics,
            );
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_record_constructor_diagnostics(guard, graph, module, diagnostics);
                }
                collect_record_constructor_diagnostics(&arm.body, graph, module, diagnostics);
            }
        }
        ExprKind::Block(block) => collect_block_diagnostics(block, graph, module, diagnostics),
        ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
    }
}

fn collect_block_diagnostics(
    block: &Block,
    graph: &ModuleGraph,
    module: ModuleId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement_diagnostics(statement, graph, module, diagnostics);
    }
}

fn collect_statement_diagnostics(
    statement: &Stmt,
    graph: &ModuleGraph,
    module: ModuleId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &statement.kind {
        StmtKind::Let {
            value: Some(value), ..
        }
        | StmtKind::Return(Some(value))
        | StmtKind::Expr(value) => {
            collect_record_constructor_diagnostics(value, graph, module, diagnostics);
        }
        StmtKind::For { iterable, body, .. } => {
            collect_record_constructor_diagnostics(iterable, graph, module, diagnostics);
            collect_block_diagnostics(body, graph, module, diagnostics);
        }
        StmtKind::Block(block) => collect_block_diagnostics(block, graph, module, diagnostics),
        StmtKind::Let { value: None, .. }
        | StmtKind::Return(None)
        | StmtKind::Break
        | StmtKind::Continue => {}
    }
}

fn collect_else_branch_diagnostics(
    branch: &ElseBranch,
    graph: &ModuleGraph,
    module: ModuleId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match branch {
        ElseBranch::Block(block) => collect_block_diagnostics(block, graph, module, diagnostics),
        ElseBranch::If(if_expr) => {
            collect_record_constructor_diagnostics(&if_expr.condition, graph, module, diagnostics);
            collect_block_diagnostics(&if_expr.then_branch, graph, module, diagnostics);
            if let Some(branch) = &if_expr.else_branch {
                collect_else_branch_diagnostics(branch, graph, module, diagnostics);
            }
        }
    }
}

fn diagnose_record_constructor(
    expr: &Expr,
    path: &[String],
    fields: &[RecordField],
    graph: &ModuleGraph,
    module: ModuleId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(declaration) = record_constructor_declaration(graph, module, path) else {
        return;
    };
    let Some(shape) = graph.struct_shape(declaration.id) else {
        return;
    };
    let explicit = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let type_name = declaration.name.as_str();
    diagnostics.extend(
        missing_required_fields(shape, &explicit)
            .into_iter()
            .map(|field| missing_field_diagnostic(type_name, field.name.as_str(), expr.span)),
    );
}

fn record_constructor_declaration<'a>(
    graph: &'a ModuleGraph,
    current_module: ModuleId,
    path: &[String],
) -> Option<&'a Declaration> {
    let name = path.last()?;
    graph.declarations().find(|declaration| {
        declaration.kind == DeclarationKind::Struct
            && declaration.name == *name
            && declaration_path_matches(graph, current_module, declaration, path)
    })
}

fn declaration_path_matches(
    graph: &ModuleGraph,
    current_module: ModuleId,
    declaration: &Declaration,
    path: &[String],
) -> bool {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return false;
    };
    if path.len() == 1 {
        return declaration.module == current_module;
    }
    let expected = path[..path.len().saturating_sub(1)].join("::");
    module_path.join() == expected
}

fn missing_required_fields<'a>(
    shape: &'a StructShape,
    explicit: &BTreeSet<&str>,
) -> Vec<&'a vela_hir::type_hint::StructFieldHint> {
    shape
        .fields
        .iter()
        .filter(|field| field.default_value_span.is_none())
        .filter(|field| !explicit.contains(field.name.as_str()))
        .collect()
}

fn missing_field_diagnostic(type_name: &str, field: &str, span: vela_common::Span) -> Diagnostic {
    Diagnostic::error(format!(
        "missing constructor field `{field}` for `{type_name}`"
    ))
    .with_code("analysis::missing_constructor_field")
    .with_span(span)
    .with_label(span, "required field is not provided and has no default")
}
