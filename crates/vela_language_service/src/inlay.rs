use std::collections::BTreeMap;

mod type_facts;

use vela_analysis::{
    registry::RegistryFacts, stdlib::stdlib_method_fact_with_lambda_arity, type_fact::TypeFact,
};
use vela_common::SourceId;
use vela_syntax::ast::{
    AstNode, SyntaxBlock, SyntaxCallExpr, SyntaxConstItem, SyntaxElseBranch, SyntaxExpression,
    SyntaxExpressionKind, SyntaxFunctionItem, SyntaxImplItem, SyntaxImplMethod, SyntaxLambdaBody,
    SyntaxMatchArmBody, SyntaxSourceFile, SyntaxStatement, SyntaxStatementKind, SyntaxTraitItem,
    SyntaxTraitMethod,
};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange, TextSize};

use crate::callable_context::{
    CallableFacts, CallableParameterFacts, callable_facts, member_callable_facts,
};
use crate::expression_facts;
use crate::symbol_ref::{builtin_member_symbol, schema_member_symbol, source_child_symbol};
use crate::{
    DiagnosticRange, DisplayParts, DocumentId, LanguageServiceDatabases, LineIndex, Position,
    SymbolRef, TextRange,
};

use self::type_facts::syntax_type_fact_from_hint;

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
    schema: &'a RegistryFacts,
}

impl<'a> TypeHintContext<'a> {
    const fn new(schema: &'a RegistryFacts) -> Self {
        Self { schema }
    }
}

struct TypeHintCollector<'a, 'hints> {
    document_id: &'a DocumentId,
    line_index: &'a LineIndex,
    range: DiagnosticRangeOffsets,
    context: TypeHintContext<'a>,
    expression_facts: &'a BTreeMap<(usize, usize), TypeFact>,
    hints: &'hints mut Vec<InlayHint>,
}

impl<'a, 'hints> TypeHintCollector<'a, 'hints> {
    fn new(
        document_id: &'a DocumentId,
        line_index: &'a LineIndex,
        range: DiagnosticRangeOffsets,
        context: TypeHintContext<'a>,
        expression_facts: &'a BTreeMap<(usize, usize), TypeFact>,
        hints: &'hints mut Vec<InlayHint>,
    ) -> Self {
        Self {
            document_id,
            line_index,
            range,
            context,
            expression_facts,
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
        let schema = self.schema_db().facts();
        let expression_facts = expression_facts::collect(graph, syntax_parse, schema);
        let mut type_collector = TypeHintCollector::new(
            document_id,
            &line_index,
            range_offsets,
            TypeHintContext::new(schema),
            &expression_facts,
            &mut hints,
        );
        type_collector.collect_source_file(syntax_parse);

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
    fn collect_source_file(&mut self, parsed: &SyntaxParse<SyntaxSourceFile>) {
        let tree = parsed.tree();
        for item in tree.items() {
            match item.syntax().kind() {
                vela_syntax::SyntaxKind::ConstItem => {
                    if let Some(item) = SyntaxConstItem::cast(item.syntax().clone())
                        && let Some(value) = item.value()
                    {
                        self.collect_expr(&value);
                    }
                }
                vela_syntax::SyntaxKind::FunctionItem => {
                    if let Some(function) = SyntaxFunctionItem::cast(item.syntax().clone()) {
                        self.collect_function(&function);
                    }
                }
                vela_syntax::SyntaxKind::ImplItem => {
                    if let Some(item) = SyntaxImplItem::cast(item.syntax().clone()) {
                        for method in item.methods() {
                            self.collect_impl_method(&method);
                        }
                    }
                }
                vela_syntax::SyntaxKind::TraitItem => {
                    if let Some(item) = SyntaxTraitItem::cast(item.syntax().clone()) {
                        for method in item.methods() {
                            self.collect_trait_method(&method);
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

    fn collect_function(&mut self, function: &SyntaxFunctionItem) {
        if let Some(params) = function.param_list() {
            for param in params.params() {
                if let Some(default) = param.default_value() {
                    self.collect_expr(&default);
                }
            }
        }
        if let Some(body) = function.body() {
            self.collect_block(&body);
        }
    }

    fn collect_impl_method(&mut self, method: &SyntaxImplMethod) {
        if let Some(params) = method.param_list() {
            for param in params.params() {
                if let Some(default) = param.default_value() {
                    self.collect_expr(&default);
                }
            }
        }
        if let Some(body) = method.body() {
            self.collect_block(&body);
        }
    }

    fn collect_trait_method(&mut self, method: &SyntaxTraitMethod) {
        if let Some(body) = method.body() {
            self.collect_block(&body);
        }
    }

    fn collect_block(&mut self, block: &SyntaxBlock) {
        for statement in block.statements() {
            self.collect_stmt(&statement);
        }
    }

    fn collect_stmt(&mut self, statement: &SyntaxStatement) {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                if let Some(statement) = statement.as_let()
                    && let Some(value) = statement.initializer()
                {
                    self.collect_expr(&value);
                    if statement.type_hint().is_none()
                        && let Some(fact) = self.expression_fact(&value)
                        && let Some(label) = type_hint_label(&fact)
                        && let Some(name_token) = statement.name_token()
                    {
                        let position_offset = text_size_to_usize(name_token.text_range().end());
                        if self.range.contains(position_offset) {
                            let name = name_token.text().to_owned();
                            let start = text_size_to_usize(name_token.text_range().start());
                            self.hints.push(InlayHint {
                                position: self.line_index.position(position_offset),
                                label,
                                kind: InlayHintKind::Type,
                                symbol: Some(SymbolRef::local_at(
                                    name,
                                    self.document_id.clone(),
                                    TextRange::new(start, position_offset),
                                )),
                            });
                        }
                    }
                }
            }
            SyntaxStatementKind::Return => {
                if let Some(expr) = statement
                    .as_return()
                    .and_then(|statement| statement.expression())
                {
                    self.collect_expr(&expr);
                }
            }
            SyntaxStatementKind::For => {
                if let Some(statement) = statement.as_for() {
                    if let Some(iterable) = statement.iterable() {
                        self.collect_expr(&iterable);
                    }
                    if let Some(body) = statement.body() {
                        self.collect_block(&body);
                    }
                }
            }
            SyntaxStatementKind::Expr => {
                if let Some(expr) = statement
                    .as_expr()
                    .and_then(|statement| statement.expression())
                {
                    self.collect_expr(&expr);
                }
            }
            SyntaxStatementKind::Block => {
                if let Some(block) = statement.as_block() {
                    self.collect_block(&block);
                }
            }
            SyntaxStatementKind::If => {
                if let Some(expr) = statement.as_if() {
                    self.collect_if(&expr);
                }
            }
            SyntaxStatementKind::Match => {
                if let Some(expr) = statement.as_match() {
                    if let Some(scrutinee) = expr.scrutinee() {
                        self.collect_expr(&scrutinee);
                    }
                    for arm in expr.arms() {
                        if let Some(guard) = arm.guard() {
                            self.collect_expr(&guard);
                        }
                        match arm.body() {
                            Some(SyntaxMatchArmBody::Expression(body)) => self.collect_expr(&body),
                            Some(SyntaxMatchArmBody::Block(body)) => self.collect_block(&body),
                            None => {}
                        }
                    }
                }
            }
            SyntaxStatementKind::Break | SyntaxStatementKind::Continue => {}
        }
    }

    fn collect_expr(&mut self, expr: &SyntaxExpression) {
        match expr.expression_kind() {
            SyntaxExpressionKind::Paren => {
                if let Some(expr) = expr.as_paren().and_then(|expr| expr.expression()) {
                    self.collect_expr(&expr);
                }
            }
            SyntaxExpressionKind::Unary => {
                if let Some(expr) = expr.as_unary().and_then(|expr| expr.expression()) {
                    self.collect_expr(&expr);
                }
            }
            SyntaxExpressionKind::Try => {
                if let Some(expr) = expr.as_try().and_then(|expr| expr.expression()) {
                    self.collect_expr(&expr);
                }
            }
            SyntaxExpressionKind::Binary => {
                if let Some(expr) = expr.as_binary() {
                    if let Some(lhs) = expr.lhs() {
                        self.collect_expr(&lhs);
                    }
                    if let Some(rhs) = expr.rhs() {
                        self.collect_expr(&rhs);
                    }
                }
            }
            SyntaxExpressionKind::Assign => {
                if let Some(expr) = expr.as_assign() {
                    if let Some(target) = expr.target() {
                        self.collect_expr(&target);
                    }
                    if let Some(value) = expr.value() {
                        self.collect_expr(&value);
                    }
                }
            }
            SyntaxExpressionKind::Field => {
                if let Some(field) = expr.as_field()
                    && let Some(base) = field.receiver()
                {
                    self.collect_expr(&base);
                    if let Some(name) = field.name_text() {
                        self.collect_field_hint(expr, &base, &name);
                    }
                }
            }
            SyntaxExpressionKind::Call => {
                if let Some(call) = expr.as_call() {
                    let lambda_params = self.lambda_parameter_facts(&call);
                    if let Some(callee) = call.callee() {
                        if let Some(base) = callee.as_field().and_then(|field| field.receiver()) {
                            self.collect_expr(&base);
                        } else {
                            self.collect_expr(&callee);
                        }
                    }
                    for arg in call.arguments() {
                        if let Some(value) = arg.expression() {
                            if value.expression_kind() == SyntaxExpressionKind::Lambda {
                                self.collect_lambda(&value, lambda_params.as_deref());
                            } else {
                                self.collect_expr(&value);
                            }
                        }
                    }
                }
            }
            SyntaxExpressionKind::Index => {
                if let Some(expr) = expr.as_index() {
                    if let Some(base) = expr.receiver() {
                        self.collect_expr(&base);
                    }
                    if let Some(index) = expr.index() {
                        self.collect_expr(&index);
                    }
                }
            }
            SyntaxExpressionKind::Array => {
                if let Some(expr) = expr.as_array() {
                    for item in expr.expressions() {
                        self.collect_expr(&item);
                    }
                }
            }
            SyntaxExpressionKind::Map => {
                if let Some(expr) = expr.as_map() {
                    for entry in expr.entries() {
                        if let Some(key) = entry.key() {
                            self.collect_expr(&key);
                        }
                        if let Some(value) = entry.value() {
                            self.collect_expr(&value);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Record => {
                if let Some(expr) = expr.as_record() {
                    for field in expr.fields() {
                        if let Some(value) = field.expression() {
                            self.collect_expr(&value);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Lambda => self.collect_lambda(expr, None),
            SyntaxExpressionKind::If => {
                if let Some(expr) = expr.as_if() {
                    self.collect_if(&expr);
                }
            }
            SyntaxExpressionKind::Match => {
                if let Some(expr) = expr.as_match() {
                    if let Some(scrutinee) = expr.scrutinee() {
                        self.collect_expr(&scrutinee);
                    }
                    for arm in expr.arms() {
                        if let Some(guard) = arm.guard() {
                            self.collect_expr(&guard);
                        }
                        match arm.body() {
                            Some(SyntaxMatchArmBody::Expression(body)) => self.collect_expr(&body),
                            Some(SyntaxMatchArmBody::Block(body)) => self.collect_block(&body),
                            None => {}
                        }
                    }
                }
            }
            SyntaxExpressionKind::Block => {
                if let Some(block) = expr.as_block() {
                    self.collect_block(&block);
                }
            }
            SyntaxExpressionKind::Literal => {
                if let Some(literal) = expr.as_literal() {
                    for interpolation in literal.interpolation_expressions() {
                        self.collect_expr(&interpolation);
                    }
                }
            }
            SyntaxExpressionKind::Path => {}
        }
    }

    fn collect_field_hint(&mut self, expr: &SyntaxExpression, base: &SyntaxExpression, name: &str) {
        let Some(TypeFact::Host { name: owner }) = self.expression_fact(base) else {
            return;
        };
        let Some(fact) = self.expression_fact(expr) else {
            return;
        };
        let Some(label) = type_hint_label(&fact) else {
            return;
        };
        let Some(field) = expr.as_field() else {
            return;
        };
        let Some(name_token) = field.name_token() else {
            return;
        };
        let position_offset = text_size_to_usize(name_token.text_range().end());
        if self.range.contains(position_offset) {
            self.hints.push(InlayHint {
                position: self.line_index.position(position_offset),
                label,
                kind: InlayHintKind::Type,
                symbol: Some(schema_member_symbol(&owner, name)),
            });
        }
    }

    fn collect_if(&mut self, if_expr: &vela_syntax::ast::SyntaxIfExpr) {
        if let Some(condition) = if_expr.condition() {
            self.collect_expr(&condition);
        }
        if let Some(then_branch) = if_expr.then_block() {
            self.collect_block(&then_branch);
        }
        match if_expr.else_branch() {
            Some(SyntaxElseBranch::If(if_expr)) => self.collect_if(&if_expr),
            Some(SyntaxElseBranch::Block(block)) => self.collect_block(&block),
            None => {}
        }
    }

    fn collect_lambda(&mut self, expr: &SyntaxExpression, inferred_params: Option<&[TypeFact]>) {
        let Some(lambda) = expr.as_lambda() else {
            return;
        };
        if let Some(params) = lambda.param_list() {
            for (index, param) in params.params().enumerate() {
                let fact = param
                    .type_hint()
                    .map(|hint| syntax_type_fact_from_hint(&hint, self.context.schema))
                    .or_else(|| inferred_params.and_then(|facts| facts.get(index).cloned()));
                if param.type_hint().is_none()
                    && let Some(fact) = fact.as_ref()
                    && let Some(label) = type_hint_label(fact)
                    && let Some(name_token) = param.name_token()
                {
                    let position_offset = text_size_to_usize(name_token.text_range().end());
                    if self.range.contains(position_offset) {
                        let start = text_size_to_usize(name_token.text_range().start());
                        self.hints.push(InlayHint {
                            position: self.line_index.position(position_offset),
                            label,
                            kind: InlayHintKind::Type,
                            symbol: Some(SymbolRef::local_at(
                                name_token.text().to_owned(),
                                self.document_id.clone(),
                                TextRange::new(start, position_offset),
                            )),
                        });
                    }
                }
                if let Some(default) = param.default_value() {
                    self.collect_expr(&default);
                }
            }
        }
        match lambda.body() {
            Some(SyntaxLambdaBody::Expression(body)) => self.collect_expr(&body),
            Some(SyntaxLambdaBody::Block(body)) => self.collect_block(&body),
            None => {}
        }
    }

    fn lambda_parameter_facts(&self, call: &SyntaxCallExpr) -> Option<Vec<TypeFact>> {
        let callee = call.callee()?;
        let field = callee.as_field()?;
        let method = field.name_text()?;
        let receiver = self.expression_fact(&field.receiver()?)?;
        let param_count = call.arguments().iter().find_map(|arg| {
            let lambda = arg.expression()?.as_lambda()?;
            Some(
                lambda
                    .param_list()
                    .map(|params| params.params().count())
                    .unwrap_or_default(),
            )
        })?;
        stdlib_method_fact_with_lambda_arity(&receiver, &method, None, Some(param_count))
            .and_then(|fact| fact.lambda.map(|lambda| lambda.params))
    }

    fn expression_fact(&self, expr: &SyntaxExpression) -> Option<TypeFact> {
        self.expression_facts
            .get(&syntax_range_key(expr.syntax().text_range()))
            .cloned()
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

fn syntax_range_key(range: SyntaxTextRange) -> (usize, usize) {
    (
        text_size_to_usize(range.start()),
        text_size_to_usize(range.end()),
    )
}

#[cfg(test)]
mod suppression_tests;
#[cfg(test)]
mod tests;
