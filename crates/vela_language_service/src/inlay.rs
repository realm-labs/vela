use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, FunctionItem, IfExpr, ItemKind, MatchExpr, Stmt,
    StmtKind,
};

use crate::{DiagnosticRange, DocumentId, LanguageServiceDatabases, LineIndex, Position};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InlayHint {
    position: Position,
    label: String,
    kind: InlayHintKind,
}

impl InlayHint {
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> InlayHintKind {
        self.kind
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn inlay_hints(&self, document_id: &DocumentId, range: DiagnosticRange) -> Vec<InlayHint> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let Some(parsed) = self.parse_db().parsed_source(document_id) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.text());
        let range_start = line_index.offset(range.start());
        let range_end = line_index.offset(range.end());
        let mut hints = Vec::new();

        for item in &parsed.items {
            match &item.kind {
                ItemKind::Const(item) => self.collect_expr_parameter_hints(
                    &item.value,
                    &line_index,
                    range_start,
                    range_end,
                    &mut hints,
                ),
                ItemKind::Function(function) => self.collect_function_parameter_hints(
                    function,
                    &line_index,
                    range_start,
                    range_end,
                    &mut hints,
                ),
                ItemKind::Impl(item) => {
                    for method in &item.methods {
                        self.collect_function_parameter_hints(
                            &method.function,
                            &line_index,
                            range_start,
                            range_end,
                            &mut hints,
                        );
                    }
                }
                ItemKind::Trait(item) => {
                    for method in &item.methods {
                        if let Some(body) = &method.default_body {
                            self.collect_block_parameter_hints(
                                body,
                                &line_index,
                                range_start,
                                range_end,
                                &mut hints,
                            );
                        }
                    }
                }
                ItemKind::Use(_)
                | ItemKind::Global(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_) => {}
            }
        }

        hints
    }

    fn collect_function_parameter_hints(
        &self,
        function: &FunctionItem,
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        for param in &function.params {
            if let Some(default) = &param.default_value {
                self.collect_expr_parameter_hints(
                    default,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
            }
        }
        self.collect_block_parameter_hints(
            &function.body,
            line_index,
            range_start,
            range_end,
            hints,
        );
    }

    fn collect_block_parameter_hints(
        &self,
        block: &Block,
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        for statement in &block.statements {
            self.collect_stmt_parameter_hints(statement, line_index, range_start, range_end, hints);
        }
    }

    fn collect_stmt_parameter_hints(
        &self,
        statement: &Stmt,
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        match &statement.kind {
            StmtKind::Let { value, .. } | StmtKind::Return(value) => {
                if let Some(expr) = value {
                    self.collect_expr_parameter_hints(
                        expr,
                        line_index,
                        range_start,
                        range_end,
                        hints,
                    );
                }
            }
            StmtKind::For { iterable, body, .. } => {
                self.collect_expr_parameter_hints(
                    iterable,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
                self.collect_block_parameter_hints(body, line_index, range_start, range_end, hints);
            }
            StmtKind::Expr(expr) => {
                self.collect_expr_parameter_hints(expr, line_index, range_start, range_end, hints);
            }
            StmtKind::Block(block) => {
                self.collect_block_parameter_hints(
                    block,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn collect_expr_parameter_hints(
        &self,
        expr: &Expr,
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        match &expr.kind {
            ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
                self.collect_expr_parameter_hints(expr, line_index, range_start, range_end, hints);
            }
            ExprKind::Binary { left, right, .. }
            | ExprKind::Assign {
                target: left,
                value: right,
                ..
            } => {
                self.collect_expr_parameter_hints(left, line_index, range_start, range_end, hints);
                self.collect_expr_parameter_hints(right, line_index, range_start, range_end, hints);
            }
            ExprKind::Field { base, .. } => {
                self.collect_expr_parameter_hints(base, line_index, range_start, range_end, hints);
            }
            ExprKind::Call { callee, args } => {
                self.collect_call_parameter_hints(
                    callee,
                    args,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
                self.collect_expr_parameter_hints(
                    callee,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
                for arg in args {
                    self.collect_expr_parameter_hints(
                        &arg.value,
                        line_index,
                        range_start,
                        range_end,
                        hints,
                    );
                }
            }
            ExprKind::Index { base, index } => {
                self.collect_expr_parameter_hints(base, line_index, range_start, range_end, hints);
                self.collect_expr_parameter_hints(index, line_index, range_start, range_end, hints);
            }
            ExprKind::Array(items) => {
                for item in items {
                    self.collect_expr_parameter_hints(
                        item,
                        line_index,
                        range_start,
                        range_end,
                        hints,
                    );
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.collect_expr_parameter_hints(
                        &entry.key,
                        line_index,
                        range_start,
                        range_end,
                        hints,
                    );
                    self.collect_expr_parameter_hints(
                        &entry.value,
                        line_index,
                        range_start,
                        range_end,
                        hints,
                    );
                }
            }
            ExprKind::Record { fields, .. } => {
                for field in fields {
                    if let Some(value) = &field.value {
                        self.collect_expr_parameter_hints(
                            value,
                            line_index,
                            range_start,
                            range_end,
                            hints,
                        );
                    }
                }
            }
            ExprKind::Lambda { params, body } => {
                for param in params {
                    if let Some(default) = &param.default_value {
                        self.collect_expr_parameter_hints(
                            default,
                            line_index,
                            range_start,
                            range_end,
                            hints,
                        );
                    }
                }
                self.collect_expr_parameter_hints(body, line_index, range_start, range_end, hints);
            }
            ExprKind::If(if_expr) => {
                self.collect_if_parameter_hints(if_expr, line_index, range_start, range_end, hints);
            }
            ExprKind::Match(match_expr) => {
                self.collect_match_parameter_hints(
                    match_expr,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
            }
            ExprKind::Block(block) => {
                self.collect_block_parameter_hints(
                    block,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                        self.collect_expr_parameter_hints(
                            expr,
                            line_index,
                            range_start,
                            range_end,
                            hints,
                        );
                    }
                }
            }
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        }
    }

    fn collect_if_parameter_hints(
        &self,
        if_expr: &IfExpr,
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        self.collect_expr_parameter_hints(
            &if_expr.condition,
            line_index,
            range_start,
            range_end,
            hints,
        );
        self.collect_block_parameter_hints(
            &if_expr.then_branch,
            line_index,
            range_start,
            range_end,
            hints,
        );
        match &if_expr.else_branch {
            Some(ElseBranch::If(if_expr)) => {
                self.collect_if_parameter_hints(if_expr, line_index, range_start, range_end, hints);
            }
            Some(ElseBranch::Block(block)) => {
                self.collect_block_parameter_hints(
                    block,
                    line_index,
                    range_start,
                    range_end,
                    hints,
                );
            }
            None => {}
        }
    }

    fn collect_match_parameter_hints(
        &self,
        match_expr: &MatchExpr,
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        self.collect_expr_parameter_hints(
            &match_expr.scrutinee,
            line_index,
            range_start,
            range_end,
            hints,
        );
        for arm in &match_expr.arms {
            if let Some(guard) = &arm.guard {
                self.collect_expr_parameter_hints(guard, line_index, range_start, range_end, hints);
            }
            self.collect_expr_parameter_hints(&arm.body, line_index, range_start, range_end, hints);
        }
    }

    fn collect_call_parameter_hints(
        &self,
        callee: &Expr,
        args: &[Argument],
        line_index: &LineIndex,
        range_start: usize,
        range_end: usize,
        hints: &mut Vec<InlayHint>,
    ) {
        let Some(callee) = callee_label(callee) else {
            return;
        };
        let Some(signature) = self.signature_candidates(&callee).into_iter().next() else {
            return;
        };

        for (index, arg) in args.iter().enumerate() {
            if arg.name.is_some() {
                continue;
            }
            let Ok(offset) = usize::try_from(arg.value.span.start) else {
                continue;
            };
            if offset < range_start || offset > range_end {
                continue;
            }
            let Some(label) = signature
                .parameters()
                .get(index)
                .and_then(parameter_hint_label)
            else {
                continue;
            };
            hints.push(InlayHint {
                position: line_index.position(offset),
                label,
                kind: InlayHintKind::Parameter,
            });
        }
    }
}

fn parameter_hint_label(parameter: &crate::SignatureParameter) -> Option<String> {
    let name = parameter.label().split(':').next()?.trim();
    (!name.is_empty()).then(|| format!("{name}:"))
}

fn callee_label(callee: &Expr) -> Option<String> {
    match &callee.kind {
        ExprKind::Path(path) => Some(path.join("::")),
        ExprKind::Field { name, .. } => Some(name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };
    use vela_analysis::type_fact::TypeFact;

    fn databases_for(files: Vec<SourceFileSnapshot>) -> LanguageServiceDatabases {
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        databases.update(&project);
        databases
    }

    #[test]
    fn inlay_hints_show_parameter_names() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn grant(amount: i64, reason: String) -> i64 { return amount }\npub fn main() { return grant(10, \"quest\") }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(1, 0), Position::new(1, 80)),
        );

        assert_eq!(
            hint_labels(&hints),
            vec![
                (Position::new(1, 29), "amount:".to_owned()),
                (Position::new(1, 33), "reason:".to_owned())
            ]
        );
        assert!(
            hints
                .iter()
                .all(|hint| hint.kind() == InlayHintKind::Parameter)
        );
    }

    #[test]
    fn inlay_hints_skip_named_arguments_and_unknown_calls() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn grant(amount: i64) -> i64 { return amount }\npub fn main() { return grant(amount = 10) + missing(1) }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(1, 0), Position::new(1, 90)),
        );

        assert!(hints.is_empty());
    }

    #[test]
    fn inlay_hints_degrade_to_any_without_schema() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { return host_grant(10) }";
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(0, 0), Position::new(0, 80)),
        );

        assert!(hints.is_empty());
    }

    #[test]
    fn inlay_hints_use_schema_function_names() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = "pub fn main() { return host_grant(10) }";
        let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let mut schema = vela_analysis::registry::RegistryFacts::default();
        schema.insert_function(
            "host_grant",
            TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
        );
        databases.set_schema_facts(schema);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(0, 0), Position::new(0, 80)),
        );

        assert_eq!(
            hint_labels(&hints),
            vec![(Position::new(0, 34), "arg0:".to_owned())]
        );
    }

    fn hint_labels(hints: &[InlayHint]) -> Vec<(Position, String)> {
        hints
            .iter()
            .map(|hint| (hint.position(), hint.label().to_owned()))
            .collect()
    }
}
