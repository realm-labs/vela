use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::StructFieldHint;
use vela_syntax::ast::{
    AstNode, SyntaxBlock, SyntaxConstItem, SyntaxExpression, SyntaxExpressionKind,
    SyntaxFunctionItem, SyntaxLambdaBody, SyntaxMatchArm, SyntaxMatchArmBody, SyntaxSourceFile,
    SyntaxStatement, SyntaxStatementKind,
};
use vela_syntax::{SyntaxKind, TextSize};

use super::{
    CompletionContext, CompletionInsertFormat, CompletionItem, CompletionKind,
    accumulator::CompletionAccumulator, display_type_detail_parts, model::RecordConstructor,
};
use crate::symbol_ref::schema_member_symbol;

pub(super) fn record_constructor_at(
    source: &SyntaxSourceFile,
    offset: usize,
) -> Option<RecordConstructor> {
    let offset = syntax_offset(offset)?;
    for item in source.items() {
        match item.syntax().kind() {
            SyntaxKind::ConstItem => {
                if let Some(item) = SyntaxConstItem::cast(item.syntax().clone())
                    && let Some(value) = item.value()
                    && let Some(context) = record_constructor_for_expr(&value, offset)
                {
                    return Some(context);
                }
            }
            SyntaxKind::FunctionItem => {
                if let Some(item) = SyntaxFunctionItem::cast(item.syntax().clone())
                    && let Some(context) = record_constructor_for_function(&item, offset)
                {
                    return Some(context);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn record_field_completion_items(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    let Some(constructor) = context.record_constructor.as_ref() else {
        return Vec::new();
    };
    let mut items = script_record_field_completions(graph, constructor);
    items.extend(schema_record_field_completions(schema, constructor));
    let existing_fields = constructor
        .field_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut accumulator = CompletionAccumulator::new(context.replace_range(), context.prefix());
    accumulator.add_many_matching(items, |item| {
        !existing_fields.contains(&item.label())
            && field_label_matches(item.label(), context.prefix())
    });
    accumulator.into_items()
}

fn record_constructor_for_function(
    function: &SyntaxFunctionItem,
    offset: TextSize,
) -> Option<RecordConstructor> {
    if let Some(params) = function.param_list() {
        for param in params.params() {
            if let Some(value) = param.default_value()
                && let Some(context) = record_constructor_for_expr(&value, offset)
            {
                return Some(context);
            }
        }
    }
    function
        .body()
        .and_then(|body| record_constructor_for_block(&body, offset))
}

fn record_constructor_for_block(
    block: &SyntaxBlock,
    offset: TextSize,
) -> Option<RecordConstructor> {
    if !block.syntax().text_range().contains(offset) {
        return None;
    }
    for statement in block.statements() {
        if let Some(context) = record_constructor_for_statement(&statement, offset) {
            return Some(context);
        }
    }
    None
}

fn record_constructor_for_statement(
    statement: &SyntaxStatement,
    offset: TextSize,
) -> Option<RecordConstructor> {
    if !statement.syntax().text_range().contains(offset) {
        return None;
    }
    match statement.statement_kind() {
        SyntaxStatementKind::Let => {
            if let Some(statement) = statement.as_let()
                && let Some(value) = statement.initializer()
            {
                return record_constructor_for_expr(&value, offset);
            }
            None
        }
        SyntaxStatementKind::Expr => {
            if let Some(statement) = statement.as_expr()
                && let Some(value) = statement.expression()
            {
                return record_constructor_for_expr(&value, offset);
            }
            None
        }
        SyntaxStatementKind::Return => {
            if let Some(statement) = statement.as_return()
                && let Some(value) = statement.expression()
            {
                return record_constructor_for_expr(&value, offset);
            }
            None
        }
        SyntaxStatementKind::Break | SyntaxStatementKind::Continue => None,
        SyntaxStatementKind::For => {
            let statement = statement.as_for()?;
            statement
                .iterable()
                .and_then(|iterable| record_constructor_for_expr(&iterable, offset))
                .or_else(|| {
                    statement
                        .body()
                        .and_then(|body| record_constructor_for_block(&body, offset))
                })
        }
        SyntaxStatementKind::Block => statement
            .as_block()
            .and_then(|block| record_constructor_for_block(&block, offset)),
        SyntaxStatementKind::If | SyntaxStatementKind::Match => {
            let expr = SyntaxExpression::cast(statement.syntax().clone())?;
            record_constructor_for_expr(&expr, offset)
        }
    }
}

fn record_constructor_for_expr(
    expr: &SyntaxExpression,
    offset: TextSize,
) -> Option<RecordConstructor> {
    if !expr.syntax().text_range().contains(offset) {
        return None;
    }
    match expr.expression_kind() {
        SyntaxExpressionKind::Record => {
            let record = expr.as_record()?;
            for field in record.fields() {
                if let Some(value) = field.expression()
                    && let Some(context) = record_constructor_for_expr(&value, offset)
                {
                    return Some(context);
                }
            }
            Some(RecordConstructor {
                path: record.path_segments(),
                field_names: record
                    .fields()
                    .into_iter()
                    .filter_map(|field| field.label_text())
                    .collect(),
                current_module: Vec::new(),
            })
        }
        SyntaxExpressionKind::Literal | SyntaxExpressionKind::Path => None,
        SyntaxExpressionKind::Paren => expr
            .as_paren()
            .and_then(|paren| paren.expression())
            .and_then(|value| record_constructor_for_expr(&value, offset)),
        SyntaxExpressionKind::Unary => expr
            .as_unary()
            .and_then(|unary| unary.expression())
            .and_then(|value| record_constructor_for_expr(&value, offset)),
        SyntaxExpressionKind::Try => expr
            .as_try()
            .and_then(|try_expr| try_expr.expression())
            .and_then(|value| record_constructor_for_expr(&value, offset)),
        SyntaxExpressionKind::Binary => {
            let binary = expr.as_binary()?;
            binary
                .lhs()
                .and_then(|value| record_constructor_for_expr(&value, offset))
                .or_else(|| {
                    binary
                        .rhs()
                        .and_then(|value| record_constructor_for_expr(&value, offset))
                })
        }
        SyntaxExpressionKind::Assign => {
            let assign = expr.as_assign()?;
            assign
                .target()
                .and_then(|value| record_constructor_for_expr(&value, offset))
                .or_else(|| {
                    assign
                        .value()
                        .and_then(|value| record_constructor_for_expr(&value, offset))
                })
        }
        SyntaxExpressionKind::Field => expr
            .as_field()
            .and_then(|field| field.receiver())
            .and_then(|value| record_constructor_for_expr(&value, offset)),
        SyntaxExpressionKind::Call => {
            let call = expr.as_call()?;
            call.callee()
                .and_then(|callee| record_constructor_for_expr(&callee, offset))
                .or_else(|| {
                    call.arguments().into_iter().find_map(|argument| {
                        argument
                            .expression()
                            .and_then(|value| record_constructor_for_expr(&value, offset))
                    })
                })
        }
        SyntaxExpressionKind::Index => {
            let index = expr.as_index()?;
            index
                .receiver()
                .and_then(|value| record_constructor_for_expr(&value, offset))
                .or_else(|| {
                    index
                        .index()
                        .and_then(|value| record_constructor_for_expr(&value, offset))
                })
        }
        SyntaxExpressionKind::Array => expr.as_array().and_then(|array| {
            array
                .expressions()
                .find_map(|value| record_constructor_for_expr(&value, offset))
        }),
        SyntaxExpressionKind::Map => expr.as_map().and_then(|map| {
            map.entries().find_map(|entry| {
                entry
                    .key()
                    .and_then(|value| record_constructor_for_expr(&value, offset))
                    .or_else(|| {
                        entry
                            .value()
                            .and_then(|value| record_constructor_for_expr(&value, offset))
                    })
            })
        }),
        SyntaxExpressionKind::Lambda => {
            let lambda = expr.as_lambda()?;
            lambda
                .param_list()
                .and_then(|params| {
                    params.params().find_map(|param| {
                        param
                            .default_value()
                            .and_then(|value| record_constructor_for_expr(&value, offset))
                    })
                })
                .or_else(|| match lambda.body() {
                    Some(SyntaxLambdaBody::Expression(value)) => {
                        record_constructor_for_expr(&value, offset)
                    }
                    Some(SyntaxLambdaBody::Block(block)) => {
                        record_constructor_for_block(&block, offset)
                    }
                    None => None,
                })
        }
        SyntaxExpressionKind::If => {
            let if_expr = expr.as_if()?;
            if_expr
                .condition()
                .and_then(|condition| record_constructor_for_expr(&condition, offset))
                .or_else(|| {
                    if_expr
                        .then_block()
                        .and_then(|block| record_constructor_for_block(&block, offset))
                })
                .or_else(|| {
                    if_expr
                        .else_as_expression()
                        .and_then(|value| record_constructor_for_expr(&value, offset))
                })
        }
        SyntaxExpressionKind::Match => {
            let match_expr = expr.as_match()?;
            match_expr
                .scrutinee()
                .and_then(|scrutinee| record_constructor_for_expr(&scrutinee, offset))
                .or_else(|| {
                    match_expr
                        .arms()
                        .into_iter()
                        .find_map(|arm| record_constructor_for_match_arm(&arm, offset))
                })
        }
        SyntaxExpressionKind::Block => expr
            .as_block()
            .and_then(|block| record_constructor_for_block(&block, offset)),
    }
}

fn record_constructor_for_match_arm(
    arm: &SyntaxMatchArm,
    offset: TextSize,
) -> Option<RecordConstructor> {
    if !arm.syntax().text_range().contains(offset) {
        return None;
    }
    arm.guard()
        .and_then(|guard| record_constructor_for_expr(&guard, offset))
        .or_else(|| match arm.body() {
            Some(SyntaxMatchArmBody::Expression(value)) => {
                record_constructor_for_expr(&value, offset)
            }
            Some(SyntaxMatchArmBody::Block(block)) => record_constructor_for_block(&block, offset),
            None => None,
        })
}

fn syntax_offset(offset: usize) -> Option<TextSize> {
    let offset = u32::try_from(offset).ok()?;
    Some(TextSize::from(offset))
}

fn script_record_field_completions(
    graph: &ModuleGraph,
    constructor: &RecordConstructor,
) -> Vec<CompletionItem> {
    let Some(declaration) = script_record_constructor_declaration(graph, constructor) else {
        return Vec::new();
    };
    let Some(shape) = graph.struct_shape(declaration.id) else {
        return Vec::new();
    };
    shape
        .fields
        .iter()
        .map(|field| field_completion_from_hint(graph, field))
        .collect()
}

fn script_record_constructor_declaration<'a>(
    graph: &'a ModuleGraph,
    constructor: &RecordConstructor,
) -> Option<&'a vela_hir::module_graph::Declaration> {
    graph.declaration_by_type_path(
        &constructor.path,
        &constructor.current_module,
        DeclarationKind::Struct,
    )
}

fn field_completion_from_hint(graph: &ModuleGraph, field: &StructFieldHint) -> CompletionItem {
    let fact = field
        .type_hint
        .as_ref()
        .map_or(TypeFact::Unknown, |hint| type_fact_from_hint(graph, hint));
    let detail_parts = display_type_detail_parts(fact.display_name());
    CompletionItem {
        label: field.name.clone(),
        kind: CompletionKind::Field,
        detail: detail_parts.render(),
        insert_text: None,
        insert_format: CompletionInsertFormat::PlainText,
        sort_text: None,
        metadata: Default::default(),
    }
    .with_detail_parts(detail_parts)
}

fn schema_record_field_completions(
    schema: &RegistryFacts,
    constructor: &RecordConstructor,
) -> Vec<CompletionItem> {
    let owner = constructor.path.join("::");
    schema
        .fields_for_owner_or_short_name(&owner)
        .into_iter()
        .map(|field| {
            let owner = field.owner;
            let name = field.name;
            let detail_parts = display_type_detail_parts(field.fact.display_name());
            CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Field,
                detail: detail_parts.render(),
                insert_text: None,
                insert_format: CompletionInsertFormat::PlainText,
                sort_text: None,
                metadata: Default::default(),
            }
            .with_detail_parts(detail_parts)
            .with_symbol(schema_member_symbol(&owner, &name))
        })
        .collect()
}

fn field_label_matches(label: &str, prefix: &str) -> bool {
    prefix.is_empty() || label.starts_with(prefix)
}
