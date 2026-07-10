use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::SourceId;
use vela_hir::body::{HirBody, HirExprKind};
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
    body: Option<&HirBody>,
    source_id: Option<SourceId>,
    offset: usize,
) -> Option<RecordConstructor> {
    body.zip(source_id)
        .and_then(|(body, source_id)| hir_record_constructor_at(body, source_id, offset))
        .or_else(|| recover_record_constructor_from_incomplete_syntax(source, offset))
}

fn hir_record_constructor_at(
    body: &HirBody,
    source_id: SourceId,
    offset: usize,
) -> Option<RecordConstructor> {
    let offset = u32::try_from(offset).ok()?;
    body.expressions
        .values()
        .filter(|expression| {
            expression.origin.span.source == source_id
                && expression.origin.span.start <= offset
                && offset <= expression.origin.span.end
                && matches!(expression.kind, HirExprKind::Record { .. })
        })
        .min_by_key(|expression| {
            expression
                .origin
                .span
                .end
                .saturating_sub(expression.origin.span.start)
        })
        .and_then(|expression| {
            let HirExprKind::Record {
                constructor,
                fields,
            } = &expression.kind
            else {
                return None;
            };
            let constructor = body.paths.get(constructor.as_ref()?)?;
            Some(RecordConstructor {
                path: constructor.path.clone(),
                field_names: fields.iter().map(|field| field.name.clone()).collect(),
                current_module: Vec::new(),
            })
        })
}

// CST traversal is deliberately confined to incomplete edits that did not
// lower a recoverable record expression into HIR.
fn recover_record_constructor_from_incomplete_syntax(
    source: &SyntaxSourceFile,
    offset: usize,
) -> Option<RecordConstructor> {
    let search = RecordConstructorSearch::new(syntax_offset(offset)?);
    for item in source.items() {
        match item.syntax().kind() {
            SyntaxKind::ConstItem => {
                if let Some(item) = SyntaxConstItem::cast(item.syntax().clone())
                    && let Some(value) = item.value()
                    && let Some(context) = record_constructor_for_expr(&value, &search)
                {
                    return Some(context);
                }
            }
            SyntaxKind::FunctionItem => {
                if let Some(item) = SyntaxFunctionItem::cast(item.syntax().clone())
                    && let Some(context) = record_constructor_for_function(&item, &search)
                {
                    return Some(context);
                }
            }
            _ => {}
        }
    }
    None
}

#[derive(Clone, Copy)]
struct RecordConstructorSearch {
    offset: TextSize,
}

impl RecordConstructorSearch {
    const fn new(offset: TextSize) -> Self {
        Self { offset }
    }
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
    search: &RecordConstructorSearch,
) -> Option<RecordConstructor> {
    if let Some(params) = function.param_list() {
        for param in params.params() {
            if let Some(value) = param.default_value()
                && let Some(context) = record_constructor_for_expr(&value, search)
            {
                return Some(context);
            }
        }
    }
    function
        .body()
        .and_then(|body| record_constructor_for_block(&body, search))
}

fn record_constructor_for_block(
    block: &SyntaxBlock,
    search: &RecordConstructorSearch,
) -> Option<RecordConstructor> {
    if !block.syntax().text_range().contains(search.offset) {
        return None;
    }
    for statement in block.statements() {
        if let Some(context) = record_constructor_for_statement(&statement, search) {
            return Some(context);
        }
    }
    None
}

fn record_constructor_for_statement(
    statement: &SyntaxStatement,
    search: &RecordConstructorSearch,
) -> Option<RecordConstructor> {
    if !statement.syntax().text_range().contains(search.offset) {
        return None;
    }
    match statement.statement_kind() {
        SyntaxStatementKind::Let => {
            if let Some(statement) = statement.as_let()
                && let Some(value) = statement.initializer()
            {
                return record_constructor_for_expr(&value, search);
            }
            None
        }
        SyntaxStatementKind::Expr => {
            if let Some(statement) = statement.as_expr()
                && let Some(value) = statement.expression()
            {
                return record_constructor_for_expr(&value, search);
            }
            None
        }
        SyntaxStatementKind::Return => {
            if let Some(statement) = statement.as_return()
                && let Some(value) = statement.expression()
            {
                return record_constructor_for_expr(&value, search);
            }
            None
        }
        SyntaxStatementKind::Break | SyntaxStatementKind::Continue => None,
        SyntaxStatementKind::For => {
            let statement = statement.as_for()?;
            statement
                .iterable()
                .and_then(|iterable| record_constructor_for_expr(&iterable, search))
                .or_else(|| {
                    statement
                        .body()
                        .and_then(|body| record_constructor_for_block(&body, search))
                })
        }
        SyntaxStatementKind::Block => statement
            .as_block()
            .and_then(|block| record_constructor_for_block(&block, search)),
        SyntaxStatementKind::If | SyntaxStatementKind::Match => {
            let expr = SyntaxExpression::cast(statement.syntax().clone())?;
            record_constructor_for_expr(&expr, search)
        }
    }
}

fn record_constructor_for_expr(
    expr: &SyntaxExpression,
    search: &RecordConstructorSearch,
) -> Option<RecordConstructor> {
    if !expr.syntax().text_range().contains(search.offset) {
        return None;
    }
    match expr.expression_kind() {
        SyntaxExpressionKind::Record => {
            let record = expr.as_record()?;
            for field in record.fields() {
                if let Some(value) = field.expression()
                    && let Some(context) = record_constructor_for_expr(&value, search)
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
        SyntaxExpressionKind::Literal | SyntaxExpressionKind::Path | SyntaxExpressionKind::Unit => {
            None
        }
        SyntaxExpressionKind::Paren => expr
            .as_paren()
            .and_then(|paren| paren.expression())
            .and_then(|value| record_constructor_for_expr(&value, search)),
        SyntaxExpressionKind::Tuple => expr.as_tuple().and_then(|tuple| {
            tuple
                .expressions()
                .find_map(|value| record_constructor_for_expr(&value, search))
        }),
        SyntaxExpressionKind::Unary => expr
            .as_unary()
            .and_then(|unary| unary.expression())
            .and_then(|value| record_constructor_for_expr(&value, search)),
        SyntaxExpressionKind::Try => expr
            .as_try()
            .and_then(|try_expr| try_expr.expression())
            .and_then(|value| record_constructor_for_expr(&value, search)),
        SyntaxExpressionKind::Binary => {
            let binary = expr.as_binary()?;
            binary
                .lhs()
                .and_then(|value| record_constructor_for_expr(&value, search))
                .or_else(|| {
                    binary
                        .rhs()
                        .and_then(|value| record_constructor_for_expr(&value, search))
                })
        }
        SyntaxExpressionKind::Assign => {
            let assign = expr.as_assign()?;
            assign
                .target()
                .and_then(|value| record_constructor_for_expr(&value, search))
                .or_else(|| {
                    assign
                        .value()
                        .and_then(|value| record_constructor_for_expr(&value, search))
                })
        }
        SyntaxExpressionKind::Field => record_constructor_for_child_exprs(expr, search),
        SyntaxExpressionKind::Call => {
            let call = expr.as_call()?;
            call.arguments()
                .into_iter()
                .find_map(|argument| {
                    argument
                        .expression()
                        .and_then(|value| record_constructor_for_expr(&value, search))
                })
                .or_else(|| record_constructor_for_child_exprs(expr, search))
        }
        SyntaxExpressionKind::Index => record_constructor_for_child_exprs(expr, search),
        SyntaxExpressionKind::Array => expr.as_array().and_then(|array| {
            array
                .expressions()
                .find_map(|value| record_constructor_for_expr(&value, search))
        }),
        SyntaxExpressionKind::Map => expr.as_map().and_then(|map| {
            map.entries().find_map(|entry| {
                entry
                    .key()
                    .and_then(|value| record_constructor_for_expr(&value, search))
                    .or_else(|| {
                        entry
                            .value()
                            .and_then(|value| record_constructor_for_expr(&value, search))
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
                            .and_then(|value| record_constructor_for_expr(&value, search))
                    })
                })
                .or_else(|| match lambda.body() {
                    Some(SyntaxLambdaBody::Expression(value)) => {
                        record_constructor_for_expr(&value, search)
                    }
                    Some(SyntaxLambdaBody::Block(block)) => {
                        record_constructor_for_block(&block, search)
                    }
                    None => None,
                })
        }
        SyntaxExpressionKind::If => {
            let if_expr = expr.as_if()?;
            if_expr
                .condition()
                .and_then(|condition| record_constructor_for_expr(&condition, search))
                .or_else(|| {
                    if_expr
                        .then_block()
                        .and_then(|block| record_constructor_for_block(&block, search))
                })
                .or_else(|| {
                    if_expr
                        .else_as_expression()
                        .and_then(|value| record_constructor_for_expr(&value, search))
                })
        }
        SyntaxExpressionKind::Match => {
            let match_expr = expr.as_match()?;
            match_expr
                .scrutinee()
                .and_then(|scrutinee| record_constructor_for_expr(&scrutinee, search))
                .or_else(|| {
                    match_expr
                        .arms()
                        .into_iter()
                        .find_map(|arm| record_constructor_for_match_arm(&arm, search))
                })
        }
        SyntaxExpressionKind::Block => expr
            .as_block()
            .and_then(|block| record_constructor_for_block(&block, search)),
    }
}

fn record_constructor_for_match_arm(
    arm: &SyntaxMatchArm,
    search: &RecordConstructorSearch,
) -> Option<RecordConstructor> {
    if !arm.syntax().text_range().contains(search.offset) {
        return None;
    }
    arm.guard()
        .and_then(|guard| record_constructor_for_expr(&guard, search))
        .or_else(|| match arm.body() {
            Some(SyntaxMatchArmBody::Expression(value)) => {
                record_constructor_for_expr(&value, search)
            }
            Some(SyntaxMatchArmBody::Block(block)) => record_constructor_for_block(&block, search),
            None => None,
        })
}

fn record_constructor_for_child_exprs(
    expr: &SyntaxExpression,
    search: &RecordConstructorSearch,
) -> Option<RecordConstructor> {
    let root_range = expr.syntax().text_range();
    expr.syntax()
        .descendants()
        .filter_map(SyntaxExpression::cast)
        .filter(|child| child.syntax().text_range() != root_range)
        .filter(|child| {
            !child
                .syntax()
                .ancestors()
                .skip(1)
                .take_while(|node| node.text_range() != root_range)
                .any(|node| SyntaxExpression::cast(node).is_some())
        })
        .find_map(|child| record_constructor_for_expr(&child, search))
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

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
    use vela_syntax::parse::parse_source_with_id;

    use super::{hir_record_constructor_at, recover_record_constructor_from_incomplete_syntax};

    #[test]
    fn hir_record_constructor_identity_does_not_depend_on_syntax_traversal() {
        let source_id = SourceId::new(1);
        let text = "pub struct Player { level: i64 }\npub fn main(players: Array<i64>) { let value = players[Player { le }]; }";
        let offset = text.find("le }").expect("record field") + 2;
        let hir_offset = u32::try_from(offset).expect("test offset should fit in u32");
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            source_id,
            ModulePath::from_qualified("game::main"),
            text,
        ));
        let body = graph
            .bodies()
            .find(|body| {
                body.expressions.values().any(|expression| {
                    expression.origin.span.source == source_id
                        && expression.origin.span.start <= hir_offset
                        && hir_offset <= expression.origin.span.end
                })
            })
            .expect("body containing record");

        let constructor = hir_record_constructor_at(body, source_id, offset)
            .expect("HIR record constructor should be available");

        assert_eq!(constructor.path, ["Player"]);
        assert_eq!(constructor.field_names, ["le"]);
    }

    #[test]
    fn incomplete_edit_recovery_is_an_explicit_syntax_boundary() {
        let source_id = SourceId::new(1);
        let text = "pub fn main() { let player = Player { le } }";
        let offset = text.find("le }").expect("field prefix") + 2;
        let parsed = parse_source_with_id(source_id, text);

        let constructor = recover_record_constructor_from_incomplete_syntax(&parsed.tree(), offset)
            .expect("incomplete record should recover from CST");

        assert_eq!(constructor.path, ["Player"]);
    }
}
