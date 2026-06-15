use vela_analysis::{
    expression::{ExprFactScope, type_fact_from_expr_with_registry},
    facts::AnalysisFacts,
    hints::type_fact_from_hint,
    registry::RegistryFacts,
    stdlib::stdlib_method_fact_with_lambda_arity,
    type_fact::TypeFact,
};
use vela_hir::module_graph::ModuleGraph;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    Argument, Block, ElseBranch, Expr, ExprKind, FunctionItem, IfExpr, ItemKind, MatchExpr, Param,
    Stmt, StmtKind, TypeHint,
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

#[derive(Clone, Copy)]
struct DiagnosticRangeOffsets {
    start: usize,
    end: usize,
}

impl DiagnosticRangeOffsets {
    const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    const fn contains(self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }
}

#[derive(Clone, Copy)]
struct TypeHintContext<'a> {
    graph: &'a ModuleGraph,
    facts: &'a AnalysisFacts,
    schema: &'a RegistryFacts,
}

impl<'a> TypeHintContext<'a> {
    const fn new(
        graph: &'a ModuleGraph,
        facts: &'a AnalysisFacts,
        schema: &'a RegistryFacts,
    ) -> Self {
        Self {
            graph,
            facts,
            schema,
        }
    }
}

struct TypeHintCollector<'a, 'hints> {
    source_text: &'a str,
    line_index: &'a LineIndex,
    range: DiagnosticRangeOffsets,
    context: TypeHintContext<'a>,
    hints: &'hints mut Vec<InlayHint>,
}

impl<'a, 'hints> TypeHintCollector<'a, 'hints> {
    fn new(
        source_text: &'a str,
        line_index: &'a LineIndex,
        range: DiagnosticRangeOffsets,
        context: TypeHintContext<'a>,
        hints: &'hints mut Vec<InlayHint>,
    ) -> Self {
        Self {
            source_text,
            line_index,
            range,
            context,
            hints,
        }
    }
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

        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let schema = self.schema_db().facts();
        let mut type_collector = TypeHintCollector::new(
            source.text(),
            &line_index,
            DiagnosticRangeOffsets::new(range_start, range_end),
            TypeHintContext::new(graph, &facts, schema),
            &mut hints,
        );
        for item in &parsed.items {
            match &item.kind {
                ItemKind::Function(function) => type_collector.collect_function(function),
                ItemKind::Impl(item) => {
                    for method in &item.methods {
                        type_collector.collect_function(&method.function);
                    }
                }
                ItemKind::Trait(item) => {
                    for method in &item.methods {
                        if let Some(body) = &method.default_body {
                            let mut scope = ExprFactScope::new();
                            type_collector.collect_block(body, &mut scope);
                        }
                    }
                }
                ItemKind::Use(_)
                | ItemKind::Const(_)
                | ItemKind::Global(_)
                | ItemKind::Struct(_)
                | ItemKind::Enum(_) => {}
            }
        }

        hints.sort_by_key(|hint| (hint.position.line, hint.position.character));
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

impl TypeHintCollector<'_, '_> {
    fn collect_function(&mut self, function: &FunctionItem) {
        let mut scope = declaration_scope(self.context);
        for param in &function.params {
            if let Some(fact) = type_fact_from_param(param, self.context) {
                scope.insert_path([param.name.clone()], fact);
            }
            if let Some(default) = &param.default_value {
                self.collect_expr(default, &mut scope);
            }
        }
        self.collect_block(&function.body, &mut scope);
    }

    fn collect_block(&mut self, block: &Block, scope: &mut ExprFactScope) {
        for statement in &block.statements {
            self.collect_stmt(statement, scope);
        }
    }

    fn collect_stmt(&mut self, statement: &Stmt, scope: &mut ExprFactScope) {
        match &statement.kind {
            StmtKind::Let {
                name,
                type_hint,
                value,
            } => {
                if let Some(value) = value {
                    self.collect_expr(value, scope);
                    let fact = type_fact_from_expr_with_registry(value, scope, self.context.schema);
                    if type_hint.is_none()
                        && let Some(label) = type_hint_label(&fact)
                        && let Some(position_offset) =
                            let_name_end_offset(statement, name, self.source_text)
                        && self.range.contains(position_offset)
                    {
                        self.hints.push(InlayHint {
                            position: self.line_index.position(position_offset),
                            label,
                            kind: InlayHintKind::Type,
                        });
                    }
                    scope.insert_path([name.clone()], fact);
                } else if let Some(type_hint) = type_hint {
                    let fact = type_fact_from_syntax_hint(type_hint, self.context);
                    scope.insert_path([name.clone()], fact);
                }
            }
            StmtKind::Return(value) => {
                if let Some(expr) = value {
                    self.collect_expr(expr, scope);
                }
            }
            StmtKind::For { iterable, body, .. } => {
                self.collect_expr(iterable, scope);
                let mut body_scope = scope.clone();
                self.collect_block(body, &mut body_scope);
            }
            StmtKind::Expr(expr) => {
                self.collect_expr(expr, scope);
            }
            StmtKind::Block(block) => {
                let mut block_scope = scope.clone();
                self.collect_block(block, &mut block_scope);
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    fn collect_expr(&mut self, expr: &Expr, scope: &mut ExprFactScope) {
        match &expr.kind {
            ExprKind::Unary { expr, .. } | ExprKind::Try(expr) => {
                self.collect_expr(expr, scope);
            }
            ExprKind::Binary { left, right, .. }
            | ExprKind::Assign {
                target: left,
                value: right,
                ..
            } => {
                self.collect_expr(left, scope);
                self.collect_expr(right, scope);
            }
            ExprKind::Field { base, name } => {
                self.collect_expr(base, scope);
                self.collect_field_hint(expr, base, name, scope);
            }
            ExprKind::Call { callee, args } => {
                let lambda_params =
                    lambda_parameter_facts(callee, args, scope, self.context.schema);
                self.collect_call_callee(callee, scope);
                for arg in args {
                    if let ExprKind::Lambda { params, body } = &arg.value.kind {
                        self.collect_lambda(params, body, scope, lambda_params.as_deref());
                    } else {
                        self.collect_expr(&arg.value, scope);
                    }
                }
            }
            ExprKind::Index { base, index } => {
                self.collect_expr(base, scope);
                self.collect_expr(index, scope);
            }
            ExprKind::Array(items) => {
                for item in items {
                    self.collect_expr(item, scope);
                }
            }
            ExprKind::Map(entries) => {
                for entry in entries {
                    self.collect_expr(&entry.key, scope);
                    self.collect_expr(&entry.value, scope);
                }
            }
            ExprKind::Record { fields, .. } => {
                for field in fields {
                    if let Some(value) = &field.value {
                        self.collect_expr(value, scope);
                    }
                }
            }
            ExprKind::Lambda { params, body } => {
                self.collect_lambda(params, body, scope, None);
            }
            ExprKind::If(if_expr) => {
                self.collect_if(if_expr, scope);
            }
            ExprKind::Match(match_expr) => {
                self.collect_expr(&match_expr.scrutinee, scope);
                for arm in &match_expr.arms {
                    let mut arm_scope = scope.clone();
                    self.collect_expr(&arm.body, &mut arm_scope);
                }
            }
            ExprKind::Block(block) => {
                let mut block_scope = scope.clone();
                self.collect_block(block, &mut block_scope);
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    if let vela_syntax::ast::InterpolatedStringPart::Expr(expr) = part {
                        self.collect_expr(expr, scope);
                    }
                }
            }
            ExprKind::Literal(_) | ExprKind::Path(_) | ExprKind::SelfValue | ExprKind::Error => {}
        }
    }

    fn collect_call_callee(&mut self, callee: &Expr, scope: &mut ExprFactScope) {
        if let ExprKind::Field { base, .. } = &callee.kind {
            self.collect_expr(base, scope);
        } else {
            self.collect_expr(callee, scope);
        }
    }

    fn collect_field_hint(&mut self, expr: &Expr, base: &Expr, name: &str, scope: &ExprFactScope) {
        let receiver = type_fact_from_expr_with_registry(base, scope, self.context.schema);
        if !matches!(receiver, TypeFact::Host { .. }) {
            return;
        }
        let fact = type_fact_from_expr_with_registry(expr, scope, self.context.schema);
        let Some(label) = type_hint_label(&fact) else {
            return;
        };
        let Some(position_offset) = field_name_end_offset(expr, name, self.source_text) else {
            return;
        };
        if self.range.contains(position_offset) {
            self.hints.push(InlayHint {
                position: self.line_index.position(position_offset),
                label,
                kind: InlayHintKind::Type,
            });
        }
    }

    fn collect_if(&mut self, if_expr: &IfExpr, scope: &mut ExprFactScope) {
        self.collect_expr(&if_expr.condition, scope);
        let mut then_scope = scope.clone();
        self.collect_block(&if_expr.then_branch, &mut then_scope);
        if let Some(else_branch) = &if_expr.else_branch {
            match else_branch {
                ElseBranch::Block(block) => {
                    let mut else_scope = scope.clone();
                    self.collect_block(block, &mut else_scope);
                }
                ElseBranch::If(nested) => {
                    let mut else_scope = scope.clone();
                    self.collect_if(nested, &mut else_scope);
                }
            }
        }
    }

    fn collect_lambda(
        &mut self,
        params: &[Param],
        body: &Expr,
        scope: &mut ExprFactScope,
        inferred_params: Option<&[TypeFact]>,
    ) {
        let mut lambda_scope = scope.clone();
        for (index, param) in params.iter().enumerate() {
            let fact = type_fact_from_param(param, self.context)
                .or_else(|| inferred_params.and_then(|facts| facts.get(index).cloned()));
            if param.type_hint.is_none()
                && let Some(fact) = fact.as_ref()
                && let Some(label) = type_hint_label(fact)
                && let Some(position_offset) = param_name_end_offset(param)
                && self.range.contains(position_offset)
            {
                self.hints.push(InlayHint {
                    position: self.line_index.position(position_offset),
                    label,
                    kind: InlayHintKind::Type,
                });
            }
            if let Some(fact) = fact {
                lambda_scope.insert_path([param.name.clone()], fact);
            }
            if let Some(default) = &param.default_value {
                self.collect_expr(default, &mut lambda_scope);
            }
        }
        self.collect_expr(body, &mut lambda_scope);
    }
}

fn parameter_hint_label(parameter: &crate::SignatureParameter) -> Option<String> {
    let name = parameter.label().split(':').next()?.trim();
    (!name.is_empty()).then(|| format!("{name}:"))
}

fn declaration_scope(context: TypeHintContext<'_>) -> ExprFactScope {
    let mut scope = ExprFactScope::new();
    for (declaration_id, fact) in context.facts.declarations() {
        let Some(declaration) = context.graph.declaration(declaration_id) else {
            continue;
        };
        scope.insert_path([declaration.name.clone()], fact.clone());
        if let Some(module_path) = context.graph.module_path(declaration.module) {
            let mut path = module_path.segments().to_vec();
            path.push(declaration.name.clone());
            scope.insert_path(path, fact.clone());
        }
    }
    scope
}

fn type_fact_from_param(param: &Param, context: TypeHintContext<'_>) -> Option<TypeFact> {
    param
        .type_hint
        .as_ref()
        .map(|hint| type_fact_from_syntax_hint(hint, context))
}

fn type_fact_from_syntax_hint(hint: &TypeHint, context: TypeHintContext<'_>) -> TypeFact {
    let fact = type_fact_from_hint(context.graph, &HirTypeHint::from_syntax(hint));
    if !matches!(fact, TypeFact::Unknown) {
        return fact;
    }

    if hint.args.is_empty() {
        let qualified = hint.path.join("::");
        context
            .schema
            .type_fact(&qualified)
            .or_else(|| {
                hint.path
                    .last()
                    .and_then(|name| context.schema.type_fact(name))
            })
            .cloned()
            .unwrap_or(TypeFact::Unknown)
    } else {
        TypeFact::Unknown
    }
}

fn lambda_parameter_facts(
    callee: &Expr,
    args: &[Argument],
    scope: &ExprFactScope,
    schema: &RegistryFacts,
) -> Option<Vec<TypeFact>> {
    let ExprKind::Field { base, name } = &callee.kind else {
        return None;
    };
    let param_count = args.iter().find_map(|arg| {
        if let ExprKind::Lambda { params, .. } = &arg.value.kind {
            Some(params.len())
        } else {
            None
        }
    })?;
    let receiver = type_fact_from_expr_with_registry(base, scope, schema);
    stdlib_method_fact_with_lambda_arity(&receiver, name, None, Some(param_count))
        .and_then(|fact| fact.lambda.map(|lambda| lambda.params))
}

fn type_hint_label(fact: &TypeFact) -> Option<String> {
    is_stable_type_fact(fact).then(|| format!(": {}", fact.display_name()))
}

fn is_stable_type_fact(fact: &TypeFact) -> bool {
    match fact {
        TypeFact::Unknown | TypeFact::Any | TypeFact::Never => false,
        TypeFact::Array { element }
        | TypeFact::Set { element }
        | TypeFact::Iterator { item: element }
        | TypeFact::Option { some: element }
        | TypeFact::OptionSome { some: element }
        | TypeFact::ResultOk { ok: element }
        | TypeFact::ResultErr { err: element } => is_stable_type_fact(element),
        TypeFact::Map { key, value }
        | TypeFact::Result {
            ok: key,
            err: value,
        } => is_stable_type_fact(key) && is_stable_type_fact(value),
        TypeFact::Function { params, returns } => {
            params.iter().all(is_stable_type_fact) && is_stable_type_fact(returns)
        }
        TypeFact::Union(facts) => {
            !facts.is_empty()
                && facts.iter().all(is_stable_type_fact)
                && facts.iter().any(|fact| !matches!(fact, TypeFact::Never))
        }
        TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::OptionNone
        | TypeFact::Record { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => true,
    }
}

fn let_name_end_offset(statement: &Stmt, name: &str, source_text: &str) -> Option<usize> {
    let start = usize::try_from(statement.span.start).ok()?;
    let end = usize::try_from(statement.span.end)
        .ok()?
        .min(source_text.len());
    let text = source_text.get(start..end)?;
    let let_offset = text.find("let")?;
    let name_start = text.get(let_offset + "let".len()..)?.find(name)?;
    Some(start + let_offset + "let".len() + name_start + name.len())
}

fn param_name_end_offset(param: &Param) -> Option<usize> {
    let start = usize::try_from(param.span.start).ok()?;
    Some(start + param.name.len())
}

fn field_name_end_offset(expr: &Expr, name: &str, source_text: &str) -> Option<usize> {
    let start = usize::try_from(expr.span.start).ok()?;
    let end = usize::try_from(expr.span.end).ok()?.min(source_text.len());
    let text = source_text.get(start..end)?;
    let name_start = text.rfind(name)?;
    Some(start + name_start + name.len())
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
    fn inlay_hints_show_stable_local_typefacts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"const BONUS: i64 = 10
pub fn main() {
    let total = 1 + 2;
    let next = total + 1;
    let scripted = BONUS;
    let explicit: i64 = 3;
    let dynamic = host_any();
}"#;
        let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let mut schema = vela_analysis::registry::RegistryFacts::default();
        schema.insert_function("host_any", TypeFact::function(Vec::new(), TypeFact::Any));
        databases.set_schema_facts(schema);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(0, 0), Position::new(7, 0)),
        );

        assert_eq!(
            hint_labels(&hints),
            vec![
                (Position::new(2, 13), ": i64".to_owned()),
                (Position::new(3, 12), ": i64".to_owned()),
                (Position::new(4, 16), ": i64".to_owned())
            ]
        );
        assert!(hints.iter().all(|hint| hint.kind() == InlayHintKind::Type));
    }

    #[test]
    fn inlay_hints_show_lambda_parameter_facts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"pub fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let doubled: Array<i64> = scores.map(|score| score + 1);
    let rewards: Map<String, i64> = {"gold": 1};
    let mapped: Map<String, i64> = rewards.map_values(|value| value + 1);
    let filtered: Map<String, i64> = rewards.filter(|key, value| key.len() > value);
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(0, 0), Position::new(7, 0)),
        );

        assert_eq!(
            hint_labels(&hints),
            vec![
                (Position::new(2, 47), ": i64".to_owned()),
                (Position::new(4, 60), ": i64".to_owned()),
                (Position::new(5, 56), ": String".to_owned()),
                (Position::new(5, 63), ": i64".to_owned())
            ]
        );
        assert!(hints.iter().all(|hint| hint.kind() == InlayHintKind::Type));
    }

    #[test]
    fn inlay_hints_show_host_path_typefacts() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"pub fn main(player: Player) {
    let next = player.level + 1;
    player.level += next;
    let dynamic = player.mystery;
    player.grant(next);
}"#;
        let mut databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);
        let mut schema = vela_analysis::registry::RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        schema.insert_field("Player", "level", TypeFact::I64);
        schema.insert_field("Player", "mystery", TypeFact::Any);
        schema.insert_method(
            "Player",
            "grant",
            TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
        );
        databases.set_schema_facts(schema);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(0, 0), Position::new(6, 0)),
        );

        assert_eq!(
            hint_labels(&hints),
            vec![
                (Position::new(1, 12), ": i64".to_owned()),
                (Position::new(1, 27), ": i64".to_owned()),
                (Position::new(2, 16), ": i64".to_owned())
            ]
        );
        assert!(hints.iter().all(|hint| hint.kind() == InlayHintKind::Type));
    }

    #[test]
    fn inlay_hints_show_enum_variant_payload_names() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"enum QuestProgress {
    Active(quest_id: String, count: i64),
    Done,
}
pub fn main() {
    let active = QuestProgress::Active("quest-1", 3);
}"#;
        let databases = databases_for(vec![SourceFileSnapshot::new(document.clone(), text)]);

        let hints = databases.inlay_hints(
            &document,
            DiagnosticRange::new(Position::new(0, 0), Position::new(7, 0)),
        );

        assert_eq!(
            hint_labels(&hints),
            vec![
                (Position::new(5, 39), "quest_id:".to_owned()),
                (Position::new(5, 50), "count:".to_owned())
            ]
        );
        assert!(
            hints
                .iter()
                .all(|hint| hint.kind() == InlayHintKind::Parameter)
        );
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
