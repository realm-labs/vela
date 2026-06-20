use vela_analysis::{
    expression::{ExprFactScope, type_fact_from_expr_with_registry},
    facts::AnalysisFacts,
    hints::type_fact_from_hint,
    registry::RegistryFacts,
    stdlib::stdlib_method_fact_with_lambda_arity,
    type_fact::TypeFact,
};
use vela_common::SourceId;
use vela_hir::module_graph::ModuleGraph;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{
    Argument, AstNode, Block, ElseBranch, Expr, ExprKind, FunctionItem, IfExpr, ItemKind, Param,
    Stmt, StmtKind, SyntaxBlock, SyntaxCallExpr, SyntaxConstItem, SyntaxElseBranch,
    SyntaxExpression, SyntaxExpressionKind, SyntaxFunctionItem, SyntaxImplItem, SyntaxImplMethod,
    SyntaxLambdaBody, SyntaxMatchArmBody, SyntaxSourceFile, SyntaxStatementKind, SyntaxTraitItem,
    SyntaxTraitMethod, TypeHint,
};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange, TextSize};

use crate::callable_context::{
    CallableFacts, CallableParameterFacts, callable_facts, member_callable_facts,
};
use crate::symbol_ref::{builtin_member_symbol, schema_member_symbol, source_child_symbol};
use crate::{
    DiagnosticRange, DisplayParts, DocumentId, LanguageServiceDatabases, LineIndex, Position,
    SymbolRef, TextRange,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InlayHint {
    position: Position,
    label: DisplayParts,
    kind: InlayHintKind,
    symbol: Option<SymbolRef>,
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
struct ParameterHintContext<'a> {
    source_id: SourceId,
    source_text: &'a str,
    line_index: &'a LineIndex,
    range: DiagnosticRangeOffsets,
}

impl<'a> ParameterHintContext<'a> {
    const fn new(
        source_id: SourceId,
        source_text: &'a str,
        line_index: &'a LineIndex,
        range: DiagnosticRangeOffsets,
    ) -> Self {
        Self {
            source_id,
            source_text,
            line_index,
            range,
        }
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
    document_id: &'a DocumentId,
    source_text: &'a str,
    line_index: &'a LineIndex,
    range: DiagnosticRangeOffsets,
    context: TypeHintContext<'a>,
    hints: &'hints mut Vec<InlayHint>,
}

impl<'a, 'hints> TypeHintCollector<'a, 'hints> {
    fn new(
        document_id: &'a DocumentId,
        source_text: &'a str,
        line_index: &'a LineIndex,
        range: DiagnosticRangeOffsets,
        context: TypeHintContext<'a>,
        hints: &'hints mut Vec<InlayHint>,
    ) -> Self {
        Self {
            document_id,
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
    pub fn label(&self) -> String {
        self.label.render()
    }

    #[must_use]
    pub fn label_parts(&self) -> &DisplayParts {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> InlayHintKind {
        self.kind
    }

    #[must_use]
    pub const fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn inlay_hints(&self, document_id: &DocumentId, range: DiagnosticRange) -> Vec<InlayHint> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let Some(syntax_parse) = self.parse_db().syntax_parse(document_id) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.text());
        let range_start = line_index.offset(range.start());
        let range_end = line_index.offset(range.end());
        let range_offsets = DiagnosticRangeOffsets::new(range_start, range_end);
        let parameter_context = ParameterHintContext::new(
            source.source_id(),
            source.text(),
            &line_index,
            range_offsets,
        );
        let mut hints = Vec::new();

        self.collect_syntax_source_parameter_hints(syntax_parse, parameter_context, &mut hints);

        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        let schema = self.schema_db().facts();
        let Some(parsed) = self.parse_db().parsed_source(document_id) else {
            hints.sort_by_key(|hint| (hint.position.line, hint.position.character));
            return hints;
        };
        let mut type_collector = TypeHintCollector::new(
            document_id,
            source.text(),
            &line_index,
            range_offsets,
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

    fn collect_syntax_source_parameter_hints(
        &self,
        parsed: &SyntaxParse<SyntaxSourceFile>,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        let tree = parsed.tree();
        for item in tree.items() {
            match item.syntax().kind() {
                vela_syntax::SyntaxKind::ConstItem => {
                    if let Some(item) = SyntaxConstItem::cast(item.syntax().clone())
                        && let Some(value) = item.value()
                    {
                        self.collect_syntax_expr_parameter_hints(&value, context, hints);
                    }
                }
                vela_syntax::SyntaxKind::FunctionItem => {
                    if let Some(function) = SyntaxFunctionItem::cast(item.syntax().clone()) {
                        self.collect_syntax_function_parameter_hints(&function, context, hints);
                    }
                }
                vela_syntax::SyntaxKind::ImplItem => {
                    if let Some(item) = SyntaxImplItem::cast(item.syntax().clone()) {
                        for method in item.methods() {
                            self.collect_syntax_impl_method_parameter_hints(
                                &method, context, hints,
                            );
                        }
                    }
                }
                vela_syntax::SyntaxKind::TraitItem => {
                    if let Some(item) = SyntaxTraitItem::cast(item.syntax().clone()) {
                        for method in item.methods() {
                            self.collect_syntax_trait_method_parameter_hints(
                                &method, context, hints,
                            );
                        }
                    }
                }
                vela_syntax::SyntaxKind::UseItem
                | vela_syntax::SyntaxKind::GlobalItem
                | vela_syntax::SyntaxKind::StructItem
                | vela_syntax::SyntaxKind::EnumItem => {}
                kind => unreachable!("non-item syntax kind: {kind:?}"),
            }
        }
    }

    fn collect_syntax_function_parameter_hints(
        &self,
        function: &SyntaxFunctionItem,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        if let Some(params) = function.param_list() {
            for param in params.params() {
                if let Some(default) = param.default_value() {
                    self.collect_syntax_expr_parameter_hints(&default, context, hints);
                }
            }
        }
        if let Some(body) = function.body() {
            self.collect_syntax_block_parameter_hints(&body, context, hints);
        }
    }

    fn collect_syntax_impl_method_parameter_hints(
        &self,
        method: &SyntaxImplMethod,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        if let Some(params) = method.param_list() {
            for param in params.params() {
                if let Some(default) = param.default_value() {
                    self.collect_syntax_expr_parameter_hints(&default, context, hints);
                }
            }
        }
        if let Some(body) = method.body() {
            self.collect_syntax_block_parameter_hints(&body, context, hints);
        }
    }

    fn collect_syntax_trait_method_parameter_hints(
        &self,
        method: &SyntaxTraitMethod,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        if let Some(body) = method.body() {
            self.collect_syntax_block_parameter_hints(&body, context, hints);
        }
    }

    fn collect_syntax_block_parameter_hints(
        &self,
        block: &SyntaxBlock,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        for statement in block.statements() {
            self.collect_syntax_stmt_parameter_hints(&statement, context, hints);
        }
    }

    fn collect_syntax_stmt_parameter_hints(
        &self,
        statement: &vela_syntax::ast::SyntaxStatement,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                if let Some(statement) = statement.as_let()
                    && let Some(expr) = statement.initializer()
                {
                    self.collect_syntax_expr_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxStatementKind::Return => {
                if let Some(statement) = statement.as_return()
                    && let Some(expr) = statement.expression()
                {
                    self.collect_syntax_expr_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxStatementKind::For => {
                if let Some(statement) = statement.as_for() {
                    if let Some(iterable) = statement.iterable() {
                        self.collect_syntax_expr_parameter_hints(&iterable, context, hints);
                    }
                    if let Some(body) = statement.body() {
                        self.collect_syntax_block_parameter_hints(&body, context, hints);
                    }
                }
            }
            SyntaxStatementKind::Expr => {
                if let Some(statement) = statement.as_expr()
                    && let Some(expr) = statement.expression()
                {
                    self.collect_syntax_expr_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxStatementKind::Block => {
                if let Some(block) = statement.as_block() {
                    self.collect_syntax_block_parameter_hints(&block, context, hints);
                }
            }
            SyntaxStatementKind::If => {
                if let Some(expr) = statement.as_if() {
                    self.collect_syntax_if_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxStatementKind::Match => {
                if let Some(expr) = statement.as_match() {
                    self.collect_syntax_match_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxStatementKind::Break | SyntaxStatementKind::Continue => {}
        }
    }

    fn collect_syntax_expr_parameter_hints(
        &self,
        expr: &SyntaxExpression,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        match expr.expression_kind() {
            SyntaxExpressionKind::Paren => {
                if let Some(expr) = expr.as_paren().and_then(|expr| expr.expression()) {
                    self.collect_syntax_expr_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxExpressionKind::Unary => {
                if let Some(expr) = expr.as_unary().and_then(|expr| expr.expression()) {
                    self.collect_syntax_expr_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxExpressionKind::Try => {
                if let Some(expr) = expr.as_try().and_then(|expr| expr.expression()) {
                    self.collect_syntax_expr_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxExpressionKind::Binary => {
                if let Some(expr) = expr.as_binary() {
                    if let Some(lhs) = expr.lhs() {
                        self.collect_syntax_expr_parameter_hints(&lhs, context, hints);
                    }
                    if let Some(rhs) = expr.rhs() {
                        self.collect_syntax_expr_parameter_hints(&rhs, context, hints);
                    }
                }
            }
            SyntaxExpressionKind::Assign => {
                if let Some(expr) = expr.as_assign() {
                    if let Some(target) = expr.target() {
                        self.collect_syntax_expr_parameter_hints(&target, context, hints);
                    }
                    if let Some(value) = expr.value() {
                        self.collect_syntax_expr_parameter_hints(&value, context, hints);
                    }
                }
            }
            SyntaxExpressionKind::Field => {
                if let Some(base) = expr.as_field().and_then(|expr| expr.receiver()) {
                    self.collect_syntax_expr_parameter_hints(&base, context, hints);
                }
            }
            SyntaxExpressionKind::Call => {
                if let Some(call) = expr.as_call() {
                    self.collect_syntax_call_parameter_hints(&call, context, hints);
                    if let Some(callee) = call.callee() {
                        self.collect_syntax_expr_parameter_hints(&callee, context, hints);
                    }
                    for arg in call.arguments() {
                        if let Some(value) = arg.expression() {
                            self.collect_syntax_expr_parameter_hints(&value, context, hints);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Index => {
                if let Some(expr) = expr.as_index() {
                    if let Some(base) = expr.receiver() {
                        self.collect_syntax_expr_parameter_hints(&base, context, hints);
                    }
                    if let Some(index) = expr.index() {
                        self.collect_syntax_expr_parameter_hints(&index, context, hints);
                    }
                }
            }
            SyntaxExpressionKind::Array => {
                if let Some(expr) = expr.as_array() {
                    for item in expr.expressions() {
                        self.collect_syntax_expr_parameter_hints(&item, context, hints);
                    }
                }
            }
            SyntaxExpressionKind::Map => {
                if let Some(expr) = expr.as_map() {
                    for entry in expr.entries() {
                        if let Some(key) = entry.key() {
                            self.collect_syntax_expr_parameter_hints(&key, context, hints);
                        }
                        if let Some(value) = entry.value() {
                            self.collect_syntax_expr_parameter_hints(&value, context, hints);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Record => {
                if let Some(expr) = expr.as_record() {
                    for field in expr.fields() {
                        if let Some(value) = field.expression() {
                            self.collect_syntax_expr_parameter_hints(&value, context, hints);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Lambda => {
                if let Some(expr) = expr.as_lambda() {
                    if let Some(params) = expr.param_list() {
                        for param in params.params() {
                            if let Some(default) = param.default_value() {
                                self.collect_syntax_expr_parameter_hints(&default, context, hints);
                            }
                        }
                    }
                    match expr.body() {
                        Some(SyntaxLambdaBody::Expression(body)) => {
                            self.collect_syntax_expr_parameter_hints(&body, context, hints);
                        }
                        Some(SyntaxLambdaBody::Block(body)) => {
                            self.collect_syntax_block_parameter_hints(&body, context, hints);
                        }
                        None => {}
                    }
                }
            }
            SyntaxExpressionKind::If => {
                if let Some(expr) = expr.as_if() {
                    self.collect_syntax_if_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxExpressionKind::Match => {
                if let Some(expr) = expr.as_match() {
                    self.collect_syntax_match_parameter_hints(&expr, context, hints);
                }
            }
            SyntaxExpressionKind::Block => {
                if let Some(block) = expr.as_block() {
                    self.collect_syntax_block_parameter_hints(&block, context, hints);
                }
            }
            SyntaxExpressionKind::Literal => {
                if let Some(literal) = expr.as_literal() {
                    for interpolation in literal.interpolation_expressions() {
                        self.collect_syntax_expr_parameter_hints(&interpolation, context, hints);
                    }
                }
            }
            SyntaxExpressionKind::Path => {}
        }
    }

    fn collect_syntax_if_parameter_hints(
        &self,
        if_expr: &vela_syntax::ast::SyntaxIfExpr,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        if let Some(condition) = if_expr.condition() {
            self.collect_syntax_expr_parameter_hints(&condition, context, hints);
        }
        if let Some(then_branch) = if_expr.then_block() {
            self.collect_syntax_block_parameter_hints(&then_branch, context, hints);
        }
        match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(if_expr)) => {
                self.collect_syntax_if_parameter_hints(&if_expr, context, hints);
            }
            Some(SyntaxElseBranch::Block(block)) => {
                self.collect_syntax_block_parameter_hints(&block, context, hints);
            }
            None => {}
        }
    }

    fn collect_syntax_match_parameter_hints(
        &self,
        match_expr: &vela_syntax::ast::SyntaxMatchExpr,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        if let Some(scrutinee) = match_expr.scrutinee() {
            self.collect_syntax_expr_parameter_hints(&scrutinee, context, hints);
        }
        for arm in match_expr.arms() {
            if let Some(guard) = arm.guard() {
                self.collect_syntax_expr_parameter_hints(&guard, context, hints);
            }
            match arm.body() {
                Some(SyntaxMatchArmBody::Expression(body)) => {
                    self.collect_syntax_expr_parameter_hints(&body, context, hints);
                }
                Some(SyntaxMatchArmBody::Block(body)) => {
                    self.collect_syntax_block_parameter_hints(&body, context, hints);
                }
                None => {}
            }
        }
    }

    fn collect_syntax_call_parameter_hints(
        &self,
        call: &SyntaxCallExpr,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        let Some(callee) = call.callee() else {
            return;
        };
        let args = call.arguments();
        let Some(callable) = self
            .syntax_call_callable_candidates(&callee, call, &args, context)
            .into_iter()
            .next()
        else {
            return;
        };

        for (index, arg) in args.iter().enumerate() {
            if arg.name_token().is_some() {
                continue;
            }
            let Some(value) = arg.expression() else {
                continue;
            };
            if value.expression_kind() == SyntaxExpressionKind::Lambda {
                continue;
            }
            let offset = text_size_to_usize(value.syntax().text_range().start());
            if !context.range.contains(offset) {
                continue;
            }
            let Some(parameter) = callable.params().get(index) else {
                continue;
            };
            let Some(label) = parameter_hint_label(parameter) else {
                continue;
            };
            hints.push(InlayHint {
                position: context.line_index.position(offset),
                label,
                kind: InlayHintKind::Parameter,
                symbol: Some(parameter_symbol(callable.symbol(), parameter.name())),
            });
        }
    }

    fn syntax_call_callable_candidates(
        &self,
        callee: &SyntaxExpression,
        call: &SyntaxCallExpr,
        args: &[vela_syntax::ast::SyntaxArgument],
        context: ParameterHintContext<'_>,
    ) -> Vec<CallableFacts> {
        if let Some((method, receiver_range)) = syntax_member_method_and_receiver_range(callee) {
            return member_callable_facts(
                self,
                context.source_id,
                receiver_range,
                &method,
                &syntax_args_prefix(call, args, context.source_text),
            );
        }

        let Some(callee) = syntax_callee_label(callee) else {
            return Vec::new();
        };
        callable_facts(self, &callee)
    }
}

fn syntax_member_method_and_receiver_range(
    callee: &SyntaxExpression,
) -> Option<(String, TextRange)> {
    let field = callee.as_field()?;
    let method = field.name_text()?;
    let receiver = field.receiver()?;
    Some((method, syntax_text_range(receiver.syntax().text_range())))
}

fn syntax_args_prefix(
    call: &SyntaxCallExpr,
    args: &[vela_syntax::ast::SyntaxArgument],
    source_text: &str,
) -> String {
    let Some(last_arg) = args.last() else {
        return String::new();
    };
    let Some(last_value) = last_arg.expression() else {
        return String::new();
    };
    let Some(open) = call
        .l_paren_token()
        .map(|token| text_size_to_usize(token.text_range().end()))
    else {
        return String::new();
    };
    let end = text_size_to_usize(last_value.syntax().text_range().end()).min(source_text.len());
    source_text
        .get(open.min(end)..end)
        .unwrap_or_default()
        .to_owned()
}

fn syntax_callee_label(callee: &SyntaxExpression) -> Option<String> {
    callee
        .as_path()
        .and_then(|path| path.path_text())
        .or_else(|| callee.as_field().and_then(|field| field.name_text()))
}

fn syntax_text_range(range: SyntaxTextRange) -> TextRange {
    TextRange::new(
        text_size_to_usize(range.start()),
        text_size_to_usize(range.end()),
    )
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
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
                            symbol: Some(SymbolRef::local_at(
                                name.clone(),
                                self.document_id.clone(),
                                TextRange::new(
                                    position_offset.saturating_sub(name.len()),
                                    position_offset,
                                ),
                            )),
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
        let TypeFact::Host { name: owner } = &receiver else {
            return;
        };
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
                symbol: Some(schema_member_symbol(owner, name)),
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
                    symbol: Some(SymbolRef::local_at(
                        param.name.clone(),
                        self.document_id.clone(),
                        TextRange::new(
                            position_offset.saturating_sub(param.name.len()),
                            position_offset,
                        ),
                    )),
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

fn parameter_hint_label(parameter: &CallableParameterFacts) -> Option<DisplayParts> {
    if !is_stable_type_fact(parameter.type_fact()) {
        return None;
    }
    let name = parameter.name();
    (!name.is_empty()).then(|| DisplayParts::parameter_hint(name))
}

fn parameter_symbol(callable: &SymbolRef, parameter: &str) -> SymbolRef {
    match callable {
        SymbolRef::Source(symbol) => source_child_symbol(symbol, parameter),
        SymbolRef::Schema(symbol) => schema_member_symbol(symbol, parameter),
        SymbolRef::Builtin(symbol) => builtin_member_symbol(symbol, parameter),
        SymbolRef::Local(symbol) => SymbolRef::local(format!("{}.{}", symbol.name(), parameter)),
    }
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
            .or_else(|| context.schema.trait_fact(&qualified))
            .or_else(|| {
                hint.path
                    .last()
                    .and_then(|name| context.schema.type_fact(name))
            })
            .or_else(|| {
                hint.path
                    .last()
                    .and_then(|name| context.schema.trait_fact(name))
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

fn type_hint_label(fact: &TypeFact) -> Option<DisplayParts> {
    is_stable_type_fact(fact).then(|| {
        let type_name = fact.display_name();
        DisplayParts::type_annotation(&type_name)
    })
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

#[cfg(test)]
mod suppression_tests;
#[cfg(test)]
mod tests;
