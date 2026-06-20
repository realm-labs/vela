use std::collections::BTreeMap;

mod owners;

use vela_analysis::{
    expression::ExprFactScope,
    registry::RegistryFacts,
    stdlib::{stdlib_function_fact, stdlib_method_fact_with_lambda_arity},
    type_fact::TypeFact,
};
use vela_common::{PrimitiveTag, SourceId};
use vela_hir::{
    module_graph::{DeclarationKind, ModuleGraph},
    type_hint::ImplMetadataKind,
};
use vela_syntax::ast::{
    AstNode, BinaryOp, Literal, SyntaxArgument, SyntaxBlock, SyntaxConstItem, SyntaxExpression,
    SyntaxExpressionKind, SyntaxFunctionItem, SyntaxIfExpr, SyntaxImplItem, SyntaxLambdaBody,
    SyntaxMatchArmBody, SyntaxParam, SyntaxSourceFile, SyntaxStatement, SyntaxStatementKind,
    SyntaxTraitItem, SyntaxTypeHint, UnaryOp,
};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange};

use crate::callable_context::query_type_fact_from_hint;
use crate::{LanguageServiceDatabases, TextRange};

use self::owners::{
    declaration_name_matches, declaration_scope, impl_target_matches, record_owner_names,
    trait_declaration_for_path, trait_owner_names,
};

pub(crate) fn collect(
    graph: &ModuleGraph,
    parsed: &SyntaxParse<SyntaxSourceFile>,
    schema: &RegistryFacts,
) -> BTreeMap<(usize, usize), TypeFact> {
    let mut collector = ExpressionFactCollector {
        graph,
        schema,
        declarations: declaration_scope(graph),
        facts: BTreeMap::new(),
    };
    collector.collect_source_file(parsed);
    collector.facts
}

pub(crate) fn fact_for_range(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    range: TextRange,
) -> Option<TypeFact> {
    let document_id =
        databases
            .source_db()
            .records()
            .iter()
            .find_map(|(document_id, source)| {
                (source.source_id() == source_id).then_some(document_id)
            })?;
    let parsed = databases.parse_db().syntax_parse(document_id)?;
    collect(
        databases.hir_db().graph(),
        parsed,
        databases.schema_db().facts(),
    )
    .get(&text_range_key(range))
    .cloned()
}

struct ExpressionFactCollector<'a> {
    graph: &'a ModuleGraph,
    schema: &'a RegistryFacts,
    declarations: ExprFactScope,
    facts: BTreeMap<(usize, usize), TypeFact>,
}

impl ExpressionFactCollector<'_> {
    fn collect_source_file(&mut self, parsed: &SyntaxParse<SyntaxSourceFile>) {
        let tree = parsed.tree();
        for item in tree.items() {
            match item.syntax().kind() {
                vela_syntax::SyntaxKind::ConstItem => {
                    if let Some(item) = SyntaxConstItem::cast(item.syntax().clone())
                        && let Some(value) = item.value()
                    {
                        let mut scope = self.root_scope();
                        self.collect_expr(&value, &mut scope);
                    }
                }
                vela_syntax::SyntaxKind::FunctionItem => {
                    if let Some(function) = SyntaxFunctionItem::cast(item.syntax().clone()) {
                        self.collect_function(&function);
                    }
                }
                vela_syntax::SyntaxKind::TraitItem => {
                    if let Some(item) = SyntaxTraitItem::cast(item.syntax().clone()) {
                        for method in item.methods() {
                            if let Some(body) = method.body() {
                                let mut scope = self.root_scope();
                                self.insert_params(&mut scope, syntax_params(method.param_list()));
                                self.collect_block(&body, &mut scope);
                            }
                        }
                    }
                }
                vela_syntax::SyntaxKind::ImplItem => {
                    if let Some(item) = SyntaxImplItem::cast(item.syntax().clone()) {
                        for method in item.methods() {
                            let mut scope = self.root_scope();
                            self.insert_params(&mut scope, syntax_params(method.param_list()));
                            if let Some(body) = method.body() {
                                self.collect_block(&body, &mut scope);
                            }
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

    fn collect_function(&mut self, item: &SyntaxFunctionItem) {
        let mut scope = self.root_scope();
        self.insert_params(&mut scope, syntax_params(item.param_list()));
        if let Some(body) = item.body() {
            self.collect_block(&body, &mut scope);
        }
    }

    fn root_scope(&self) -> ExprFactScope {
        self.declarations.clone()
    }

    fn insert_params(
        &self,
        scope: &mut ExprFactScope,
        params: impl IntoIterator<Item = SyntaxParam>,
    ) {
        for param in params {
            if let Some(name) = param.name_text() {
                if let Some(type_hint) = param.type_hint() {
                    scope.insert_path([name.clone()], self.type_fact_from_hint(&type_hint));
                }
                if let Some(default) = param.default_value() {
                    let fact = self.type_fact_for_expr(&default, scope);
                    if !matches!(fact, TypeFact::Unknown) {
                        scope.insert_path([name], fact);
                    }
                }
            }
        }
    }

    fn collect_block(&mut self, block: &SyntaxBlock, scope: &mut ExprFactScope) {
        for statement in block.statements() {
            self.collect_stmt(&statement, scope);
        }
    }

    fn collect_stmt(&mut self, statement: &SyntaxStatement, scope: &mut ExprFactScope) {
        match statement.statement_kind() {
            SyntaxStatementKind::Let => {
                let Some(statement) = statement.as_let() else {
                    return;
                };
                let Some(name) = statement.name_text() else {
                    return;
                };
                if let Some(value) = statement.initializer() {
                    self.collect_expr(&value, scope);
                    let fact = self.type_fact_for_expr(&value, scope);
                    if !matches!(fact, TypeFact::Unknown) {
                        scope.insert_path([name], fact);
                    }
                } else if let Some(type_hint) = statement.type_hint() {
                    scope.insert_path([name], self.type_fact_from_hint(&type_hint));
                }
            }
            SyntaxStatementKind::Return => {
                if let Some(expr) = statement
                    .as_return()
                    .and_then(|statement| statement.expression())
                {
                    self.collect_expr(&expr, scope);
                }
            }
            SyntaxStatementKind::For => {
                if let Some(statement) = statement.as_for() {
                    if let Some(iterable) = statement.iterable() {
                        self.collect_expr(&iterable, scope);
                    }
                    if let Some(body) = statement.body() {
                        let mut body_scope = scope.clone();
                        self.collect_block(&body, &mut body_scope);
                    }
                }
            }
            SyntaxStatementKind::Expr => {
                if let Some(expr) = statement
                    .as_expr()
                    .and_then(|statement| statement.expression())
                {
                    self.collect_expr(&expr, scope);
                }
            }
            SyntaxStatementKind::Block => {
                if let Some(block) = statement.as_block() {
                    let mut block_scope = scope.clone();
                    self.collect_block(&block, &mut block_scope);
                }
            }
            SyntaxStatementKind::If => {
                if let Some(if_expr) = statement.as_if() {
                    self.collect_if(&if_expr, scope);
                }
            }
            SyntaxStatementKind::Match => {
                if let Some(match_expr) = statement.as_match() {
                    if let Some(scrutinee) = match_expr.scrutinee() {
                        self.collect_expr(&scrutinee, scope);
                    }
                    for arm in match_expr.arms() {
                        let mut arm_scope = scope.clone();
                        if let Some(guard) = arm.guard() {
                            self.collect_expr(&guard, &mut arm_scope);
                        }
                        match arm.body() {
                            Some(SyntaxMatchArmBody::Expression(body)) => {
                                self.collect_expr(&body, &mut arm_scope);
                            }
                            Some(SyntaxMatchArmBody::Block(body)) => {
                                self.collect_block(&body, &mut arm_scope);
                            }
                            None => {}
                        }
                    }
                }
            }
            SyntaxStatementKind::Break | SyntaxStatementKind::Continue => {}
        }
    }

    fn collect_expr(&mut self, expr: &SyntaxExpression, scope: &mut ExprFactScope) {
        match expr.expression_kind() {
            SyntaxExpressionKind::Paren => {
                if let Some(expr) = expr.as_paren().and_then(|expr| expr.expression()) {
                    self.collect_expr(&expr, scope);
                }
            }
            SyntaxExpressionKind::Unary => {
                if let Some(expr) = expr.as_unary().and_then(|expr| expr.expression()) {
                    self.collect_expr(&expr, scope);
                }
            }
            SyntaxExpressionKind::Try => {
                if let Some(expr) = expr.as_try().and_then(|expr| expr.expression()) {
                    self.collect_expr(&expr, scope);
                }
            }
            SyntaxExpressionKind::Binary => {
                if let Some(expr) = expr.as_binary() {
                    if let Some(lhs) = expr.lhs() {
                        self.collect_expr(&lhs, scope);
                    }
                    if let Some(rhs) = expr.rhs() {
                        self.collect_expr(&rhs, scope);
                    }
                }
            }
            SyntaxExpressionKind::Assign => {
                if let Some(expr) = expr.as_assign() {
                    if let Some(target) = expr.target() {
                        self.collect_expr(&target, scope);
                    }
                    if let Some(value) = expr.value() {
                        self.collect_expr(&value, scope);
                    }
                }
            }
            SyntaxExpressionKind::Field => {
                if let Some(base) = expr.as_field().and_then(|expr| expr.receiver()) {
                    self.collect_expr(&base, scope);
                }
            }
            SyntaxExpressionKind::Call => {
                if let Some(call) = expr.as_call() {
                    if let Some(callee) = call.callee() {
                        self.collect_expr(&callee, scope);
                    }
                    for arg in call.arguments() {
                        if let Some(value) = arg.expression() {
                            self.collect_expr(&value, scope);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Index => {
                if let Some(expr) = expr.as_index() {
                    if let Some(base) = expr.receiver() {
                        self.collect_expr(&base, scope);
                    }
                    if let Some(index) = expr.index() {
                        self.collect_expr(&index, scope);
                    }
                }
            }
            SyntaxExpressionKind::Array => {
                if let Some(expr) = expr.as_array() {
                    for item in expr.expressions() {
                        self.collect_expr(&item, scope);
                    }
                }
            }
            SyntaxExpressionKind::Map => {
                if let Some(expr) = expr.as_map() {
                    for entry in expr.entries() {
                        if let Some(key) = entry.key() {
                            self.collect_expr(&key, scope);
                        }
                        if let Some(value) = entry.value() {
                            self.collect_expr(&value, scope);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Record => {
                if let Some(expr) = expr.as_record() {
                    for field in expr.fields() {
                        if let Some(value) = field.expression() {
                            self.collect_expr(&value, scope);
                        }
                    }
                }
            }
            SyntaxExpressionKind::Lambda => {
                if let Some(expr) = expr.as_lambda() {
                    let mut lambda_scope = scope.clone();
                    if let Some(params) = expr.param_list() {
                        self.insert_params(&mut lambda_scope, params.params());
                    }
                    match expr.body() {
                        Some(SyntaxLambdaBody::Expression(body)) => {
                            self.collect_expr(&body, &mut lambda_scope);
                        }
                        Some(SyntaxLambdaBody::Block(body)) => {
                            self.collect_block(&body, &mut lambda_scope);
                        }
                        None => {}
                    }
                }
            }
            SyntaxExpressionKind::If => {
                if let Some(if_expr) = expr.as_if() {
                    self.collect_if(&if_expr, scope);
                }
            }
            SyntaxExpressionKind::Match => {
                if let Some(match_expr) = expr.as_match() {
                    if let Some(scrutinee) = match_expr.scrutinee() {
                        self.collect_expr(&scrutinee, scope);
                    }
                    for arm in match_expr.arms() {
                        let mut arm_scope = scope.clone();
                        if let Some(guard) = arm.guard() {
                            self.collect_expr(&guard, &mut arm_scope);
                        }
                        match arm.body() {
                            Some(SyntaxMatchArmBody::Expression(body)) => {
                                self.collect_expr(&body, &mut arm_scope);
                            }
                            Some(SyntaxMatchArmBody::Block(body)) => {
                                self.collect_block(&body, &mut arm_scope);
                            }
                            None => {}
                        }
                    }
                }
            }
            SyntaxExpressionKind::Block => {
                if let Some(block) = expr.as_block() {
                    let mut block_scope = scope.clone();
                    self.collect_block(&block, &mut block_scope);
                }
            }
            SyntaxExpressionKind::Literal => {
                if let Some(literal) = expr.as_literal() {
                    for interpolation in literal.interpolation_expressions() {
                        self.collect_expr(&interpolation, scope);
                    }
                }
            }
            SyntaxExpressionKind::Path => {}
        }

        let fact = self.type_fact_for_expr(expr, scope);
        if !matches!(fact, TypeFact::Unknown) {
            self.facts
                .insert(syntax_range_key(expr.syntax().text_range()), fact);
        }
    }

    fn collect_if(&mut self, if_expr: &SyntaxIfExpr, scope: &mut ExprFactScope) {
        if let Some(condition) = if_expr.condition() {
            self.collect_expr(&condition, scope);
        }
        if let Some(then_branch) = if_expr.then_block() {
            let mut then_scope = scope.clone();
            self.collect_block(&then_branch, &mut then_scope);
        }
        match if_expr.else_branch() {
            Some(vela_syntax::ast::SyntaxElseBranch::If(nested)) => {
                let mut else_scope = scope.clone();
                self.collect_if(&nested, &mut else_scope);
            }
            Some(vela_syntax::ast::SyntaxElseBranch::Block(block)) => {
                let mut else_scope = scope.clone();
                self.collect_block(&block, &mut else_scope);
            }
            None => {}
        }
    }

    fn type_fact_for_expr(&self, expr: &SyntaxExpression, scope: &ExprFactScope) -> TypeFact {
        let fact = self.type_fact_from_expr(expr, scope);
        if !matches!(fact, TypeFact::Unknown) {
            return fact;
        }
        let Some(call) = expr.as_call() else {
            return fact;
        };
        let Some(callee) = call.callee() else {
            return fact;
        };
        self.source_call_return_fact(&callee, scope).unwrap_or(fact)
    }

    fn type_fact_from_expr(&self, expr: &SyntaxExpression, scope: &ExprFactScope) -> TypeFact {
        match expr.expression_kind() {
            SyntaxExpressionKind::Literal => expr
                .as_literal()
                .and_then(|literal| literal.literal())
                .map_or(TypeFact::Unknown, literal_fact),
            SyntaxExpressionKind::Path => expr
                .as_path()
                .and_then(|path| {
                    if path.is_self() {
                        return scope.path_fact(&["self".to_owned()]).cloned();
                    }
                    let segments = path.path_segments();
                    scope
                        .path_fact(&segments)
                        .cloned()
                        .or_else(|| path_field_fact(&segments, scope, self.schema))
                })
                .unwrap_or(TypeFact::Unknown),
            SyntaxExpressionKind::Paren => expr
                .as_paren()
                .and_then(|expr| expr.expression())
                .map_or(TypeFact::Unknown, |expr| {
                    self.type_fact_from_expr(&expr, scope)
                }),
            SyntaxExpressionKind::Unary => expr
                .as_unary()
                .and_then(|expr| {
                    let op = expr.operator()?;
                    let value = expr.expression()?;
                    Some(unary_fact(op, self.type_fact_from_expr(&value, scope)))
                })
                .unwrap_or(TypeFact::Unknown),
            SyntaxExpressionKind::Binary => expr
                .as_binary()
                .and_then(|expr| {
                    let op = expr.operator()?;
                    let left = expr.lhs()?;
                    let right = expr.rhs()?;
                    Some(binary_fact(
                        op,
                        self.type_fact_from_expr(&left, scope),
                        self.type_fact_from_expr(&right, scope),
                    ))
                })
                .unwrap_or(TypeFact::Unknown),
            SyntaxExpressionKind::Assign => expr
                .as_assign()
                .and_then(|expr| expr.value())
                .map_or(TypeFact::Unknown, |value| {
                    self.type_fact_from_expr(&value, scope)
                }),
            SyntaxExpressionKind::Try => expr
                .as_try()
                .and_then(|expr| expr.expression())
                .map_or(TypeFact::Unknown, |value| {
                    try_fact(self.type_fact_from_expr(&value, scope))
                }),
            SyntaxExpressionKind::Field => expr
                .as_field()
                .and_then(|expr| {
                    let base = expr.receiver()?;
                    let name = expr.name_text()?;
                    Some(field_access_fact(
                        self.type_fact_from_expr(&base, scope),
                        &name,
                        self.schema,
                    ))
                })
                .unwrap_or(TypeFact::Unknown),
            SyntaxExpressionKind::Index => expr
                .as_index()
                .and_then(|expr| {
                    let base = expr.receiver()?;
                    let index = expr.index()?;
                    Some(index_fact(
                        self.type_fact_from_expr(&base, scope),
                        self.type_fact_from_expr(&index, scope),
                        self.schema,
                    ))
                })
                .unwrap_or(TypeFact::Unknown),
            SyntaxExpressionKind::Call => expr
                .as_call()
                .map_or(TypeFact::Unknown, |call| self.call_fact(&call, scope)),
            SyntaxExpressionKind::Array => expr.as_array().map_or(TypeFact::Unknown, |expr| {
                TypeFact::array(collection_fact(
                    expr.expressions()
                        .map(|value| self.type_fact_from_expr(&value, scope)),
                ))
            }),
            SyntaxExpressionKind::Map => expr.as_map().map_or(TypeFact::Unknown, |expr| {
                let entries = expr.entries().collect::<Vec<_>>();
                let key = collection_fact(
                    entries
                        .iter()
                        .filter_map(|entry| entry.key().map(|key| self.map_key_fact(&key, scope))),
                );
                let value = collection_fact(entries.iter().filter_map(|entry| {
                    entry
                        .value()
                        .map(|value| self.type_fact_from_expr(&value, scope))
                }));
                TypeFact::map(key, value)
            }),
            SyntaxExpressionKind::Record => expr
                .as_record()
                .map(|expr| TypeFact::record(expr.path_segments().join("::")))
                .unwrap_or(TypeFact::Unknown),
            SyntaxExpressionKind::Lambda => expr.as_lambda().map_or(TypeFact::Unknown, |expr| {
                self.lambda_fact(syntax_params(expr.param_list()), expr.body(), scope, None)
            }),
            SyntaxExpressionKind::If => expr.as_if().map_or(TypeFact::Unknown, |if_expr| {
                self.if_expr_fact(&if_expr, scope)
            }),
            SyntaxExpressionKind::Match => expr.as_match().map_or(TypeFact::Unknown, |expr| {
                TypeFact::union(expr.arms().into_iter().filter_map(|arm| {
                    let arm_scope = scope.clone();
                    match arm.body() {
                        Some(SyntaxMatchArmBody::Expression(body)) => {
                            Some(self.type_fact_from_expr(&body, &arm_scope))
                        }
                        Some(SyntaxMatchArmBody::Block(block)) => {
                            Some(self.block_fact(&block, &arm_scope))
                        }
                        None => None,
                    }
                }))
            }),
            SyntaxExpressionKind::Block => expr
                .as_block()
                .map_or(TypeFact::Unknown, |block| self.block_fact(&block, scope)),
        }
    }

    fn call_fact(
        &self,
        call: &vela_syntax::ast::SyntaxCallExpr,
        scope: &ExprFactScope,
    ) -> TypeFact {
        let Some(callee) = call.callee() else {
            return TypeFact::Unknown;
        };
        let args = call.arguments();
        match callee.expression_kind() {
            SyntaxExpressionKind::Path => {
                let Some(path) = callee.as_path() else {
                    return TypeFact::Unknown;
                };
                let segments = path.path_segments();
                let arg_facts = args
                    .iter()
                    .filter_map(|arg| {
                        arg.expression()
                            .map(|expr| self.type_fact_from_expr(&expr, scope))
                    })
                    .collect::<Vec<_>>();
                if let Some(fact) = stdlib_function_fact(&segments.join("::"), &arg_facts) {
                    return fact.returns;
                }
                if let Some(fact) = self
                    .schema
                    .function_fact(&segments.join("::"))
                    .and_then(function_return_fact)
                {
                    return fact;
                }
                if let Some(fact) = scope.path_fact(&segments).and_then(function_return_fact) {
                    return fact;
                }

                let Some((method, receiver_path)) = segments.split_last() else {
                    return TypeFact::Unknown;
                };
                let receiver = scope
                    .path_fact(receiver_path)
                    .cloned()
                    .unwrap_or(TypeFact::Unknown);
                if let Some(fact) = registry_method_return_fact(&receiver, method, self.schema) {
                    return fact;
                }
                let lambda_return = args.first().and_then(|arg| {
                    arg.expression()
                        .and_then(|expr| self.lambda_return_fact(&receiver, method, &expr, scope))
                });
                stdlib_method_fact_with_lambda_arity(
                    &receiver,
                    method,
                    lambda_return.as_ref(),
                    first_lambda_param_count(&args),
                )
                .map_or(TypeFact::Unknown, |fact| fact.returns)
            }
            SyntaxExpressionKind::Field => {
                let Some(field) = callee.as_field() else {
                    return TypeFact::Unknown;
                };
                let Some(base) = field.receiver() else {
                    return TypeFact::Unknown;
                };
                let Some(name) = field.name_text() else {
                    return TypeFact::Unknown;
                };
                let receiver = self.type_fact_from_expr(&base, scope);
                if let Some(fact) = registry_method_return_fact(&receiver, &name, self.schema) {
                    return fact;
                }
                let lambda_return = args.first().and_then(|arg| {
                    arg.expression()
                        .and_then(|expr| self.lambda_return_fact(&receiver, &name, &expr, scope))
                });
                stdlib_method_fact_with_lambda_arity(
                    &receiver,
                    &name,
                    lambda_return.as_ref(),
                    first_lambda_param_count(&args),
                )
                .map_or(TypeFact::Unknown, |fact| fact.returns)
            }
            _ => TypeFact::Unknown,
        }
    }

    fn source_call_return_fact(
        &self,
        callee: &SyntaxExpression,
        scope: &ExprFactScope,
    ) -> Option<TypeFact> {
        let field = callee.as_field()?;
        let receiver = self.type_fact_from_expr(&field.receiver()?, scope);
        self.source_method_return_fact(&receiver, &field.name_text()?)
    }

    fn source_method_return_fact(&self, receiver: &TypeFact, method: &str) -> Option<TypeFact> {
        self.source_impl_method_return_fact(receiver, method)
            .or_else(|| self.source_trait_method_return_fact(receiver, method))
            .or_else(|| self.source_trait_default_method_return_fact(receiver, method))
    }

    fn source_impl_method_return_fact(
        &self,
        receiver: &TypeFact,
        method: &str,
    ) -> Option<TypeFact> {
        let owner_names = record_owner_names(receiver);
        self.graph.declarations().find_map(|declaration| {
            if declaration.kind != DeclarationKind::Impl {
                return None;
            }
            let metadata = self.graph.impl_metadata(declaration.id)?;
            let matches_owner = owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner));
            if !matches_owner {
                return None;
            }
            let method = metadata.methods.iter().find(|entry| entry.name == method)?;
            method
                .signature
                .return_type
                .as_ref()
                .map(|hint| query_type_fact_from_hint(self.graph, hint, self.schema))
        })
    }

    fn source_trait_method_return_fact(
        &self,
        receiver: &TypeFact,
        method: &str,
    ) -> Option<TypeFact> {
        let owner_names = trait_owner_names(receiver);
        self.graph.declarations().find_map(|declaration| {
            if declaration.kind != DeclarationKind::Trait
                || !owner_names
                    .iter()
                    .any(|owner| declaration_name_matches(self.graph, declaration.id, owner))
            {
                return None;
            }
            let method = self
                .graph
                .trait_shape(declaration.id)?
                .methods
                .iter()
                .find(|entry| entry.name == method)?;
            method
                .signature
                .return_type
                .as_ref()
                .map(|hint| query_type_fact_from_hint(self.graph, hint, self.schema))
        })
    }

    fn source_trait_default_method_return_fact(
        &self,
        receiver: &TypeFact,
        method: &str,
    ) -> Option<TypeFact> {
        let owner_names = record_owner_names(receiver);
        self.graph.declarations().find_map(|declaration| {
            if declaration.kind != DeclarationKind::Impl {
                return None;
            }
            let metadata = self.graph.impl_metadata(declaration.id)?;
            let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
                return None;
            };
            let matches_owner = owner_names
                .iter()
                .any(|owner| impl_target_matches(&metadata.target_path, owner));
            if !matches_owner || metadata.methods.iter().any(|entry| entry.name == method) {
                return None;
            }
            let trait_declaration = trait_declaration_for_path(self.graph, trait_path)?;
            let method = self
                .graph
                .trait_shape(trait_declaration)?
                .methods
                .iter()
                .find(|entry| entry.name == method && entry.has_default)?;
            method
                .signature
                .return_type
                .as_ref()
                .map(|hint| query_type_fact_from_hint(self.graph, hint, self.schema))
        })
    }

    fn map_key_fact(&self, key: &SyntaxExpression, scope: &ExprFactScope) -> TypeFact {
        match key.expression_kind() {
            SyntaxExpressionKind::Literal | SyntaxExpressionKind::Path => TypeFact::STRING,
            _ => self.type_fact_from_expr(key, scope),
        }
    }

    fn lambda_return_fact(
        &self,
        receiver: &TypeFact,
        method: &str,
        expr: &SyntaxExpression,
        scope: &ExprFactScope,
    ) -> Option<TypeFact> {
        let lambda = expr.as_lambda()?;
        let params = syntax_params(lambda.param_list());
        let inferred_params =
            stdlib_method_fact_with_lambda_arity(receiver, method, None, Some(params.len()))
                .and_then(|fact| fact.lambda.map(|lambda| lambda.params));
        let TypeFact::Function { returns, .. } =
            self.lambda_fact(params, lambda.body(), scope, inferred_params)
        else {
            return None;
        };
        Some(*returns)
    }

    fn lambda_fact(
        &self,
        params: Vec<SyntaxParam>,
        body: Option<SyntaxLambdaBody>,
        scope: &ExprFactScope,
        inferred_params: Option<Vec<TypeFact>>,
    ) -> TypeFact {
        let mut nested = scope.clone();
        let mut param_facts = Vec::new();

        for (index, param) in params.into_iter().enumerate() {
            let fact = param
                .type_hint()
                .map(|hint| self.type_fact_from_hint(&hint))
                .or_else(|| {
                    inferred_params
                        .as_ref()
                        .and_then(|facts| facts.get(index).cloned())
                })
                .unwrap_or(TypeFact::Unknown);
            if let Some(name) = param.name_text() {
                nested.insert_path([name], fact.clone());
            }
            param_facts.push(fact);
        }

        let returns = match body {
            Some(SyntaxLambdaBody::Expression(body)) => self.type_fact_from_expr(&body, &nested),
            Some(SyntaxLambdaBody::Block(body)) => self.block_fact(&body, &nested),
            None => TypeFact::Unknown,
        };
        TypeFact::function(param_facts, returns)
    }

    fn block_fact(&self, block: &SyntaxBlock, scope: &ExprFactScope) -> TypeFact {
        block
            .statements()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find_map(|statement| match statement.statement_kind() {
                SyntaxStatementKind::Return => statement
                    .as_return()
                    .and_then(|statement| statement.expression())
                    .map(|expr| self.type_fact_from_expr(&expr, scope)),
                SyntaxStatementKind::Expr => statement
                    .as_expr()
                    .and_then(|statement| statement.expression())
                    .map(|expr| self.type_fact_from_expr(&expr, scope)),
                SyntaxStatementKind::Block => statement
                    .as_block()
                    .map(|block| self.block_fact(&block, scope)),
                _ => None,
            })
            .unwrap_or(TypeFact::NULL)
    }

    fn if_expr_fact(&self, if_expr: &SyntaxIfExpr, scope: &ExprFactScope) -> TypeFact {
        let mut branch_facts = Vec::new();
        if let Some(then_branch) = if_expr.then_block() {
            branch_facts.push(self.block_fact(&then_branch, scope));
        }
        branch_facts.push(if_expr.else_branch().map_or(TypeFact::NULL, |else_branch| {
            match else_branch {
                vela_syntax::ast::SyntaxElseBranch::If(if_expr) => {
                    self.if_expr_fact(&if_expr, scope)
                }
                vela_syntax::ast::SyntaxElseBranch::Block(block) => self.block_fact(&block, scope),
            }
        }));
        TypeFact::union(branch_facts)
    }

    fn type_fact_from_hint(&self, hint: &SyntaxTypeHint) -> TypeFact {
        if hint
            .type_arg_list()
            .is_none_or(|args| args.type_hints().next().is_none())
        {
            let segments = hint.path_segments();
            let qualified = segments.join("::");
            self.schema
                .type_fact(&qualified)
                .or_else(|| self.schema.trait_fact(&qualified))
                .or_else(|| segments.last().and_then(|name| self.schema.type_fact(name)))
                .or_else(|| {
                    segments
                        .last()
                        .and_then(|name| self.schema.trait_fact(name))
                })
                .cloned()
                .unwrap_or_else(|| type_fact_from_syntax_hint(hint))
        } else {
            type_fact_from_syntax_hint(hint)
        }
    }
}

fn literal_fact(literal: Literal) -> TypeFact {
    match literal {
        Literal::Null => TypeFact::NULL,
        Literal::Bool(_) => TypeFact::BOOL,
        Literal::Char(_) => TypeFact::CHAR,
        Literal::Integer(_) => TypeFact::I64,
        Literal::Float(_) => TypeFact::F64,
        Literal::String(_) => TypeFact::STRING,
        Literal::Bytes(_) => TypeFact::BYTES,
    }
}

fn unary_fact(op: UnaryOp, operand: TypeFact) -> TypeFact {
    match op {
        UnaryOp::Not => TypeFact::BOOL,
        UnaryOp::Negate => match operand {
            TypeFact::Primitive(PrimitiveTag::I64 | PrimitiveTag::F64) => operand,
            _ => TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]),
        },
    }
}

fn binary_fact(op: BinaryOp, left: TypeFact, right: TypeFact) -> TypeFact {
    match op {
        BinaryOp::Or
        | BinaryOp::And
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::IdentityEqual
        | BinaryOp::IdentityNotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual => TypeFact::BOOL,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            numeric_result([left, right])
        }
        BinaryOp::Range | BinaryOp::RangeInclusive => TypeFact::Range,
    }
}

fn numeric_result(values: impl IntoIterator<Item = TypeFact>) -> TypeFact {
    let values = values.into_iter().collect::<Vec<_>>();
    if values
        .iter()
        .all(|value| matches!(value, TypeFact::Primitive(PrimitiveTag::I64)))
    {
        TypeFact::I64
    } else if values.iter().all(|value| {
        matches!(
            value,
            TypeFact::Primitive(PrimitiveTag::I64 | PrimitiveTag::F64)
        )
    }) {
        TypeFact::F64
    } else {
        TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])
    }
}

fn try_fact(value: TypeFact) -> TypeFact {
    match value {
        TypeFact::Option { some } | TypeFact::OptionSome { some } => *some,
        TypeFact::OptionNone => TypeFact::Never,
        TypeFact::Result { ok, .. } | TypeFact::ResultOk { ok } => *ok,
        TypeFact::ResultErr { .. } => TypeFact::Never,
        TypeFact::Union(facts) => TypeFact::union(facts.into_iter().map(try_fact)),
        _ => TypeFact::Unknown,
    }
}

fn registry_method_return_fact(
    receiver: &TypeFact,
    method: &str,
    facts: &RegistryFacts,
) -> Option<TypeFact> {
    registry_owner_names(receiver).iter().find_map(|owner| {
        facts
            .method_fact(owner, method)
            .or_else(|| facts.trait_method_fact(owner, method))
            .and_then(function_return_fact)
    })
}

fn function_return_fact(fact: &TypeFact) -> Option<TypeFact> {
    match fact {
        TypeFact::Function { returns, .. } => Some((**returns).clone()),
        _ => None,
    }
}

fn registry_owner_names(receiver: &TypeFact) -> Vec<String> {
    match receiver {
        TypeFact::Host { name }
        | TypeFact::Record { name }
        | TypeFact::Trait { name }
        | TypeFact::Enum { name, .. } => vec![name.clone()],
        TypeFact::Union(facts) => facts
            .iter()
            .flat_map(registry_owner_names)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}

fn first_lambda_param_count(args: &[SyntaxArgument]) -> Option<usize> {
    let lambda = args.first()?.expression()?.as_lambda()?;
    Some(syntax_params(lambda.param_list()).len())
}

fn path_field_fact(
    path: &[String],
    scope: &ExprFactScope,
    facts: &RegistryFacts,
) -> Option<TypeFact> {
    let (field, receiver_path) = path.split_last()?;
    if receiver_path.is_empty() {
        return None;
    }
    let receiver = scope.path_fact(receiver_path)?;
    registry_field_fact(receiver, field, facts)
}

fn field_access_fact(receiver: TypeFact, field: &str, facts: &RegistryFacts) -> TypeFact {
    registry_field_fact(&receiver, field, facts).unwrap_or(TypeFact::Unknown)
}

fn registry_field_fact(
    receiver: &TypeFact,
    field: &str,
    facts: &RegistryFacts,
) -> Option<TypeFact> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            facts.field_fact(name, field).cloned()
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => facts
            .field_fact(&format!("{name}::{variant}"), field)
            .cloned(),
        _ => None,
    }
}

fn index_fact(base: TypeFact, index: TypeFact, facts: &RegistryFacts) -> TypeFact {
    match base {
        TypeFact::Array { element } if accepts_int_index(&index) => *element,
        TypeFact::Map { key, value } if accepts_map_key(&index, &key) => *value,
        TypeFact::Host { name } => facts
            .index_capability_fact(&name)
            .filter(|capability| capability.readable && accepts_map_key(&index, &capability.key))
            .map_or(TypeFact::Unknown, |capability| capability.value.clone()),
        TypeFact::Union(members) => TypeFact::union(
            members
                .into_iter()
                .map(|fact| index_fact(fact, index.clone(), facts))
                .filter(|fact| !matches!(fact, TypeFact::Unknown)),
        ),
        _ => TypeFact::Unknown,
    }
}

fn accepts_int_index(index: &TypeFact) -> bool {
    match index {
        TypeFact::Primitive(PrimitiveTag::I64) | TypeFact::Any | TypeFact::Unknown => true,
        TypeFact::Union(facts) => facts.iter().any(accepts_int_index),
        _ => false,
    }
}

fn accepts_map_key(index: &TypeFact, key: &TypeFact) -> bool {
    match (index, key) {
        (TypeFact::Any | TypeFact::Unknown, _) | (_, TypeFact::Any | TypeFact::Unknown) => true,
        (TypeFact::Union(facts), key) => facts.iter().any(|fact| accepts_map_key(fact, key)),
        (index, TypeFact::Union(facts)) => facts.iter().any(|fact| accepts_map_key(index, fact)),
        _ => key == index,
    }
}

fn collection_fact(facts: impl IntoIterator<Item = TypeFact>) -> TypeFact {
    TypeFact::union(facts)
}

fn syntax_params(param_list: Option<vela_syntax::ast::SyntaxParamList>) -> Vec<SyntaxParam> {
    param_list
        .map(|params| params.params().collect())
        .unwrap_or_default()
}

fn type_fact_from_syntax_hint(hint: &SyntaxTypeHint) -> TypeFact {
    let args = hint
        .type_arg_list()
        .map(|args| args.type_hints().collect::<Vec<_>>())
        .unwrap_or_default();
    match hint.path_segments().as_slice() {
        [name] => {
            if name == "Array" && args.len() == 1 {
                return TypeFact::array(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Map" && args.len() == 2 {
                return TypeFact::map(
                    type_fact_from_syntax_hint(&args[0]),
                    type_fact_from_syntax_hint(&args[1]),
                );
            }
            if name == "Set" && args.len() == 1 {
                return TypeFact::set(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Iterator" && args.len() == 1 {
                return TypeFact::iterator(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Option" && args.len() == 1 {
                return TypeFact::option(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Result" && args.len() == 2 {
                return TypeFact::result(
                    type_fact_from_syntax_hint(&args[0]),
                    type_fact_from_syntax_hint(&args[1]),
                );
            }
            if let Some(tag) = PrimitiveTag::from_name(name) {
                return TypeFact::primitive(tag);
            }

            match name.as_str() {
                "Any" => TypeFact::Any,
                "String" => TypeFact::primitive(PrimitiveTag::String),
                "Bytes" => TypeFact::primitive(PrimitiveTag::Bytes),
                "Array" => TypeFact::array(TypeFact::Unknown),
                "Map" => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
                "Set" => TypeFact::set(TypeFact::Unknown),
                "Iterator" => TypeFact::iterator(TypeFact::Unknown),
                "Function" => TypeFact::function(Vec::new(), TypeFact::Unknown),
                "Option" => TypeFact::option(TypeFact::Unknown),
                "Result" => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
                name => TypeFact::record(name),
            }
        }
        path => TypeFact::record(path.join("::")),
    }
}

fn text_range_key(range: TextRange) -> (usize, usize) {
    (range.start, range.end)
}

fn syntax_range_key(range: SyntaxTextRange) -> (usize, usize) {
    (
        u32::from(range.start()) as usize,
        u32::from(range.end()) as usize,
    )
}
