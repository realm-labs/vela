use vela_common::{SourceId, Span};

use crate::ast::{
    AstNode, SyntaxArrayExpr, SyntaxAssignExpr, SyntaxBreakStmt, SyntaxCallExpr, SyntaxConstItem,
    SyntaxContinueStmt, SyntaxExprStmt, SyntaxForStmt, SyntaxGlobalItem, SyntaxIfExpr,
    SyntaxIndexExpr, SyntaxLambdaExpr, SyntaxMapExpr, SyntaxMatchExpr, SyntaxRecordExpr,
    SyntaxRecordPattern, SyntaxReturnStmt, SyntaxTryExpr, SyntaxTuplePattern, SyntaxUnaryExpr,
    SyntaxUseItem,
};
use crate::parse::parse_source_with_id;
use crate::{SyntaxKind, TextRange, TextSize};

mod item_boundaries;

#[test]
fn parser_parse_source_builds_lossless_source_file_root() {
    let source = "#!/usr/bin/env vela\n// hello\nfn main() { return 1; }\n";
    let parse = parse_source_with_id(SourceId::new(7), source);
    let tree = parse.tree();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(tree.syntax().kind(), SyntaxKind::SourceFile);
    assert_eq!(tree.syntax().text().to_string(), source);
    assert_eq!(
        tree.syntax()
            .children()
            .map(|node| node.kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::FunctionItem]
    );
    assert_eq!(
        tree.text_range(),
        TextRange::new(TextSize::from(0), TextSize::of(source))
    );
}

#[test]
fn parser_parse_source_wraps_top_level_items_in_cst_nodes() {
    let source =
        "# [event(\"tick\")]\npub fn tick() {}\nuse game::state;\nstruct Player { level: i64 }\n";
    let parse = parse_source_with_id(SourceId::new(11), source);
    let tree = parse.tree();
    let function = tree.functions().next().expect("function item");
    let attributes = function.attributes().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(tree.syntax().text().to_string(), source);
    assert_eq!(
        tree.items()
            .map(|item| item.syntax().kind())
            .collect::<Vec<_>>(),
        vec![
            SyntaxKind::FunctionItem,
            SyntaxKind::UseItem,
            SyntaxKind::StructItem,
        ]
    );
    assert_eq!(attributes.len(), 1);
    assert_eq!(attributes[0].syntax().kind(), SyntaxKind::Attribute);
    assert_eq!(
        attributes[0].syntax().text().to_string(),
        "# [event(\"tick\")]"
    );
    assert_eq!(attributes[0].path_text().as_deref(), Some("event"));
}

#[test]
fn parser_parse_source_reports_missing_function_name() {
    let source = "fn () {}\nfn grant(amount: i64) -> i64 { return amount; }\n";
    let parse = parse_source_with_id(SourceId::new(12), source);
    let tree = parse.tree();

    assert_eq!(
        tree.items()
            .map(|item| item.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::FunctionItem, SyntaxKind::FunctionItem]
    );
    assert!(
        parse.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("E_PARSE")
                && diagnostic.message == "expected function name"
        }),
        "{:?}",
        parse.diagnostics()
    );
}

#[test]
fn parser_parse_source_structures_use_const_and_global_items() {
    let source = r#"use game::state::Player as PlayerState;
const DEFAULT_LEVEL: i64 = base_level + 1;
global current_player: Player;
"#;
    let parse = parse_source_with_id(SourceId::new(22), source);
    let tree = parse.tree();
    let use_item = tree.uses().next().expect("use item");
    let const_item = tree.consts().next().expect("const item");
    let global_item = tree.globals().next().expect("global item");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(tree.syntax().text().to_string(), source);
    assert_eq!(
        tree.items()
            .map(|item| item.syntax().kind())
            .collect::<Vec<_>>(),
        vec![
            SyntaxKind::UseItem,
            SyntaxKind::ConstItem,
            SyntaxKind::GlobalItem,
        ]
    );
    assert_eq!(
        SyntaxUseItem::cast(use_item.syntax().clone())
            .expect("typed use item")
            .path()
            .expect("use path")
            .syntax()
            .text()
            .to_string(),
        "game::state::Player"
    );
    assert_eq!(
        SyntaxConstItem::cast(const_item.syntax().clone())
            .expect("typed const item")
            .type_hint()
            .expect("const type hint")
            .syntax()
            .text()
            .to_string(),
        "i64"
    );
    assert_eq!(
        const_item.value().expect("const value").syntax().kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(
        SyntaxGlobalItem::cast(global_item.syntax().clone())
            .expect("typed global item")
            .type_hint()
            .expect("global type hint")
            .syntax()
            .text()
            .to_string(),
        "Player"
    );
}

#[test]
fn parser_parse_source_structures_function_signature_and_body_nodes() {
    let source = "fn award(ctx: Context, items: Array<String>, amount = bonus(1, 2)) -> Result<Map<String, i64>, String> { return amount; }\n";
    let parse = parse_source_with_id(SourceId::new(12), source);
    let tree = parse.tree();
    let function = tree.functions().next().expect("function item");
    let params = function.param_list().expect("param list");
    let body = function.body().expect("body");
    let params_vec = params.params().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(function.syntax().text().to_string(), source.trim_end());
    assert_eq!(params.syntax().kind(), SyntaxKind::ParamList);
    assert_eq!(
        params_vec
            .iter()
            .map(|param| param.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec![
            "ctx: Context",
            " items: Array<String>",
            " amount = bonus(1, 2)",
        ]
    );
    assert_eq!(
        params_vec[0]
            .type_hint()
            .expect("ctx type")
            .syntax()
            .text()
            .to_string(),
        "Context"
    );
    let items_hint = params_vec[1].type_hint().expect("items type");
    assert_eq!(items_hint.syntax().text().to_string(), "Array<String>");
    let items_args = items_hint.type_arg_list().expect("items type args");
    assert_eq!(items_args.syntax().text().to_string(), "<String>");
    assert_eq!(
        items_args
            .type_hints()
            .map(|hint| hint.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["String"]
    );
    assert!(params_vec[2].type_hint().is_none());
    let return_type = function.return_type().expect("return type");
    assert_eq!(
        return_type.syntax().text().to_string(),
        "Result<Map<String, i64>, String>"
    );
    let return_args = return_type.type_arg_list().expect("return type args");
    assert_eq!(
        return_args.syntax().text().to_string(),
        "<Map<String, i64>, String>"
    );
    let return_arg_hints = return_args.type_hints().collect::<Vec<_>>();
    assert_eq!(
        return_arg_hints
            .iter()
            .map(|hint| hint.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["Map<String, i64>", "String"]
    );
    assert_eq!(
        return_arg_hints[0]
            .type_arg_list()
            .expect("nested map args")
            .type_hints()
            .map(|hint| hint.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["String", "i64"]
    );
    assert_eq!(body.syntax().kind(), SyntaxKind::Block);
    assert_eq!(body.syntax().text().to_string(), "{ return amount; }");
}

#[test]
fn parser_parse_source_structures_struct_field_nodes() {
    let source = r#"struct Reward {
    #[doc("Reward item")]
    item_id: String = "gold",
    count: i64 = 1
    tags: Array<String>
}
"#;
    let parse = parse_source_with_id(SourceId::new(13), source);
    let tree = parse.tree();
    let record = tree.structs().next().expect("struct item");
    let field_list = record.field_list().expect("field list");
    let fields = field_list.fields().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(record.syntax().text().to_string(), source.trim_end());
    assert_eq!(field_list.syntax().kind(), SyntaxKind::StructFieldList);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec![
            "#[doc(\"Reward item\")]\n    item_id: String = \"gold\"",
            "count: i64 = 1",
            "tags: Array<String>",
        ]
    );
    let field_attrs = fields[0].attributes().collect::<Vec<_>>();
    assert_eq!(field_attrs.len(), 1);
    assert_eq!(field_attrs[0].path_text().as_deref(), Some("doc"));
    assert_eq!(
        fields[0]
            .type_hint()
            .expect("item type")
            .syntax()
            .text()
            .to_string(),
        "String"
    );
    let tags_hint = fields[2].type_hint().expect("tags type");
    assert_eq!(tags_hint.syntax().text().to_string(), "Array<String>");
    assert_eq!(
        tags_hint
            .type_arg_list()
            .expect("tags type args")
            .syntax()
            .text()
            .to_string(),
        "<String>"
    );
}

#[test]
fn parser_parse_source_recovers_same_line_struct_fields_without_comma() {
    let source = "struct Player { id: String level: i64 }";
    let parse = parse_source_with_id(SourceId::new(13), source);
    let tree = parse.tree();
    let record = tree.structs().next().expect("struct item");
    let fields = record
        .field_list()
        .expect("field list")
        .fields()
        .collect::<Vec<_>>();

    assert_eq!(
        fields
            .iter()
            .map(|field| field.name_text().expect("field name"))
            .collect::<Vec<_>>(),
        vec!["id", "level"]
    );
    assert_eq!(
        fields
            .iter()
            .map(|field| {
                field
                    .type_hint()
                    .expect("field type")
                    .path_text()
                    .expect("type path")
            })
            .collect::<Vec<_>>(),
        vec!["String", "i64"]
    );
}

#[test]
fn parser_parse_source_structures_enum_variant_nodes() {
    let source = r#"enum QuestProgress {
    #[empty]
    None
    Active { quest_id: String, count: i64 = 0 }
    Finished(quest_id: String, reward: Option<String>)
}
"#;
    let parse = parse_source_with_id(SourceId::new(14), source);
    let tree = parse.tree();
    let enumeration = tree.enums().next().expect("enum item");
    let variant_list = enumeration.variant_list().expect("variant list");
    let variants = variant_list.variants().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(enumeration.syntax().text().to_string(), source.trim_end());
    assert_eq!(variant_list.syntax().kind(), SyntaxKind::EnumVariantList);
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec![
            "#[empty]\n    None",
            "Active { quest_id: String, count: i64 = 0 }",
            "Finished(quest_id: String, reward: Option<String>)",
        ]
    );
    assert!(variants[0].tuple_field_list().is_none());
    assert!(variants[0].record_field_list().is_none());

    let record_fields = variants[1]
        .record_field_list()
        .expect("record fields")
        .fields()
        .collect::<Vec<_>>();
    assert_eq!(
        record_fields
            .iter()
            .map(|field| field.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["quest_id: String", "count: i64 = 0"]
    );
    assert_eq!(
        record_fields[0]
            .type_hint()
            .expect("record field type")
            .syntax()
            .text()
            .to_string(),
        "String"
    );

    let tuple_params = variants[2]
        .tuple_field_list()
        .expect("tuple fields")
        .params()
        .collect::<Vec<_>>();
    assert_eq!(
        tuple_params
            .iter()
            .map(|param| param.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["quest_id: String", " reward: Option<String>"]
    );
    let reward_hint = tuple_params[1].type_hint().expect("tuple field type");
    assert_eq!(reward_hint.syntax().text().to_string(), "Option<String>");
    assert_eq!(
        reward_hint
            .type_arg_list()
            .expect("tuple type args")
            .syntax()
            .text()
            .to_string(),
        "<String>"
    );
}

#[test]
fn parser_parse_source_structures_trait_and_impl_method_nodes() {
    let source = r#"trait Rewardable {
    #[doc("Reward method")]
    fn reward(ctx: Context, amount: i64) -> Result<String, String>;
    fn fallback(ctx: Context) { return "fallback"; }
}

impl Rewardable for Player {
    #[trace]
    fn reward(ctx: Context, amount: i64) -> Result<String, String> { return "gold"; }
}
"#;
    let parse = parse_source_with_id(SourceId::new(15), source);
    let tree = parse.tree();
    let trait_item = tree.traits().next().expect("trait item");
    let trait_methods = trait_item.methods().collect::<Vec<_>>();
    let impl_item = tree.impls().next().expect("impl item");
    let impl_methods = impl_item.methods().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        tree.items()
            .map(|item| item.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::TraitItem, SyntaxKind::ImplItem]
    );
    assert_eq!(
        trait_methods
            .iter()
            .map(|method| method.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec![
            "#[doc(\"Reward method\")]\n    fn reward(ctx: Context, amount: i64) -> Result<String, String>;",
            "fn fallback(ctx: Context) { return \"fallback\"; }",
        ]
    );
    let reward_params = trait_methods[0]
        .param_list()
        .expect("trait method params")
        .params()
        .collect::<Vec<_>>();
    assert_eq!(
        reward_params
            .iter()
            .map(|param| param.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["ctx: Context", " amount: i64"]
    );
    let reward_return = trait_methods[0]
        .return_type()
        .expect("trait method return type");
    assert_eq!(
        reward_return.syntax().text().to_string(),
        "Result<String, String>"
    );
    assert_eq!(
        reward_return
            .type_arg_list()
            .expect("trait method return args")
            .syntax()
            .text()
            .to_string(),
        "<String, String>"
    );
    assert!(trait_methods[0].body().is_none());
    assert_eq!(
        trait_methods[1]
            .body()
            .expect("trait default body")
            .syntax()
            .text()
            .to_string(),
        "{ return \"fallback\"; }"
    );

    assert_eq!(impl_methods.len(), 1);
    assert_eq!(
        impl_methods[0].syntax().text().to_string(),
        "#[trace]\n    fn reward(ctx: Context, amount: i64) -> Result<String, String> { return \"gold\"; }"
    );
    assert_eq!(
        impl_methods[0]
            .body()
            .expect("impl body")
            .syntax()
            .text()
            .to_string(),
        "{ return \"gold\"; }"
    );
    assert_eq!(
        impl_methods[0]
            .return_type()
            .expect("impl return type")
            .syntax()
            .text()
            .to_string(),
        "Result<String, String>"
    );
}

#[test]
fn parser_parse_source_structures_block_statement_nodes() {
    let source = r#"fn update(ctx: Context) {
    let score: i64 = 1;
    return score;
    for item in items {
        let nested: String = "gold";
        continue;
    }
    if score > 1 {
        break;
    } else if score == 1 {
        return score;
    } else {
        score += 1;
    }
    match score {
        _ => score,
    }
    score += 1;
}
"#;
    let parse = parse_source_with_id(SourceId::new(16), source);
    let tree = parse.tree();
    let function = tree.functions().next().expect("function item");
    let body = function.body().expect("function body");
    let statements = body.statements().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(function.syntax().text().to_string(), source.trim_end());
    assert_eq!(
        statements
            .iter()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![
            SyntaxKind::LetStmt,
            SyntaxKind::ReturnStmt,
            SyntaxKind::ForStmt,
            SyntaxKind::IfExpr,
            SyntaxKind::MatchExpr,
            SyntaxKind::ExprStmt,
        ]
    );
    let let_stmt = body.let_statements().next().expect("let statement");
    assert_eq!(
        let_stmt
            .type_hint()
            .expect("let type hint")
            .syntax()
            .text()
            .to_string(),
        "i64"
    );

    let for_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxForStmt::cast)
        .expect("for statement");
    let for_body = for_stmt.body().expect("for body");
    assert_eq!(
        for_body
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::LetStmt, SyntaxKind::ContinueStmt]
    );
    let continue_stmt = for_body
        .syntax()
        .children()
        .find_map(SyntaxContinueStmt::cast)
        .expect("continue statement");
    assert_eq!(continue_stmt.syntax().text().to_string(), "continue;");
    assert_eq!(
        for_body
            .let_statements()
            .next()
            .expect("nested let")
            .type_hint()
            .expect("nested let type")
            .syntax()
            .text()
            .to_string(),
        "String"
    );

    let if_expr = body
        .syntax()
        .children()
        .find_map(SyntaxIfExpr::cast)
        .expect("if expression");
    assert_eq!(
        if_expr.condition().expect("if condition").syntax().kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(
        if_expr
            .then_block()
            .expect("if then block")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::BreakStmt]
    );
    assert!(if_expr.else_block().is_none());
    let if_block = if_expr.blocks().next().expect("if block");
    let break_stmt = if_block
        .syntax()
        .children()
        .find_map(SyntaxBreakStmt::cast)
        .expect("break statement");
    assert_eq!(break_stmt.syntax().text().to_string(), "break;");
    assert_eq!(if_expr.blocks().count(), 1);
    let else_if = if_expr.else_if().expect("else-if expression");
    assert_eq!(
        else_if
            .condition()
            .expect("else-if condition")
            .syntax()
            .kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(else_if.blocks().count(), 2);
    assert_eq!(
        else_if
            .then_block()
            .expect("else-if then block")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ReturnStmt]
    );
    assert_eq!(
        else_if
            .else_block()
            .expect("else block")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ExprStmt]
    );
}

#[test]
fn parser_parse_source_structures_statement_expression_nodes() {
    let source = r#"fn update(ctx: Context) {
    let score = award(player.level, amount = 1);
    let penalty = -score;
    let ready = !done;
    return score;
    #[trace]
    player.level += award(score, 1);
}
"#;
    let parse = parse_source_with_id(SourceId::new(17), source);
    let tree = parse.tree();
    let body = tree
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(tree.syntax().text().to_string(), source);

    let let_statements = body.let_statements().collect::<Vec<_>>();
    let initializer = let_statements[0].initializer().expect("initializer");
    assert_eq!(initializer.syntax().kind(), SyntaxKind::CallExpr);
    let initializer_call =
        SyntaxCallExpr::cast(initializer.syntax().clone()).expect("initializer call");
    assert_eq!(
        initializer_call
            .arg_list()
            .expect("initializer args")
            .arguments()
            .map(|argument| argument
                .expression()
                .expect("argument expression")
                .syntax()
                .kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::FieldExpr, SyntaxKind::Literal]
    );
    let negative = SyntaxUnaryExpr::cast(
        let_statements[1]
            .initializer()
            .expect("negative initializer")
            .syntax()
            .clone(),
    )
    .expect("negative unary expression");
    assert_eq!(
        negative
            .expression()
            .expect("negative operand")
            .syntax()
            .kind(),
        SyntaxKind::PathExpr
    );
    let inverted = SyntaxUnaryExpr::cast(
        let_statements[2]
            .initializer()
            .expect("inverted initializer")
            .syntax()
            .clone(),
    )
    .expect("inverted unary expression");
    assert_eq!(
        inverted
            .expression()
            .expect("inverted operand")
            .syntax()
            .kind(),
        SyntaxKind::PathExpr
    );

    let return_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxReturnStmt::cast)
        .expect("return statement");
    assert_eq!(
        return_stmt
            .expression()
            .expect("return expression")
            .syntax()
            .kind(),
        SyntaxKind::PathExpr
    );

    let expr_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxExprStmt::cast)
        .expect("expression statement");
    assert_eq!(
        expr_stmt
            .attributes()
            .next()
            .expect("expr statement attribute")
            .path_text()
            .as_deref(),
        Some("trace")
    );
    let assignment = SyntaxAssignExpr::cast(expr_stmt.expression().expect("expr").syntax().clone())
        .expect("assignment expression");
    assert_eq!(
        assignment
            .expressions()
            .map(|expression| expression.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::FieldExpr, SyntaxKind::CallExpr]
    );
}

#[test]
fn parser_parse_source_structures_postfix_expression_nodes() {
    let source = r#"fn update(ctx: Context) {
    let item = player.inventory[find_slot(score)]?;
    player.reward(item);
}
"#;
    let parse = parse_source_with_id(SourceId::new(18), source);
    let tree = parse.tree();
    let body = tree
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(tree.syntax().text().to_string(), source);

    let try_expr = SyntaxTryExpr::cast(
        body.let_statements()
            .next()
            .expect("let statement")
            .initializer()
            .expect("initializer")
            .syntax()
            .clone(),
    )
    .expect("try expression");
    let index_expr =
        SyntaxIndexExpr::cast(try_expr.expression().expect("try operand").syntax().clone())
            .expect("index expression");
    assert_eq!(
        index_expr
            .expressions()
            .map(|expression| expression.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::FieldExpr, SyntaxKind::CallExpr]
    );

    let call_expr = body
        .syntax()
        .children()
        .find_map(SyntaxExprStmt::cast)
        .and_then(|statement| statement.expression())
        .and_then(|expression| SyntaxCallExpr::cast(expression.syntax().clone()))
        .expect("method call expression");
    assert_eq!(
        call_expr.callee().expect("method callee").syntax().kind(),
        SyntaxKind::FieldExpr
    );
    assert_eq!(
        call_expr
            .arg_list()
            .expect("method args")
            .arguments()
            .count(),
        1
    );
}

#[test]
fn parser_parse_source_structures_container_and_lambda_expression_nodes() {
    let source = r#"fn update(ctx: Context) {
    let values = [1, score + 2, player.level];
    let table = { "score": score, player: player.level };
    let reward = Reward { item_id: item, count, tags: [item] };
    let doubled = |value: i64| value * 2;
    let from_block = |value| { return value; };
}
"#;
    let parse = parse_source_with_id(SourceId::new(19), source);
    let tree = parse.tree();
    let body = tree
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let lets = body.let_statements().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(tree.syntax().text().to_string(), source);
    assert_eq!(lets.len(), 5);

    let array_expr = SyntaxArrayExpr::cast(lets[0].initializer().expect("array").syntax().clone())
        .expect("array expression");
    assert_eq!(
        array_expr
            .expressions()
            .map(|expression| expression.syntax().kind())
            .collect::<Vec<_>>(),
        vec![
            SyntaxKind::Literal,
            SyntaxKind::BinaryExpr,
            SyntaxKind::FieldExpr,
        ]
    );

    let map_expr = SyntaxMapExpr::cast(lets[1].initializer().expect("map").syntax().clone())
        .expect("map expression");
    assert_eq!(
        map_expr
            .entries()
            .map(|entry| entry
                .expressions()
                .map(|expression| expression.syntax().kind())
                .collect::<Vec<_>>())
            .collect::<Vec<Vec<_>>>(),
        vec![
            vec![SyntaxKind::Literal, SyntaxKind::PathExpr],
            vec![SyntaxKind::PathExpr, SyntaxKind::FieldExpr],
        ]
    );

    let record_expr =
        SyntaxRecordExpr::cast(lets[2].initializer().expect("record").syntax().clone())
            .expect("record expression");
    assert_eq!(
        record_expr
            .path()
            .expect("record path")
            .syntax()
            .text()
            .to_string(),
        "Reward"
    );
    let record_fields = record_expr
        .field_list()
        .expect("record fields")
        .fields()
        .collect::<Vec<_>>();
    assert_eq!(record_fields.len(), 3);
    assert_eq!(
        record_fields
            .iter()
            .map(|field| field
                .expression()
                .map(|expression| expression.syntax().kind()))
            .collect::<Vec<_>>(),
        vec![
            Some(SyntaxKind::PathExpr),
            None,
            Some(SyntaxKind::ArrayExpr),
        ]
    );

    let lambda_expr =
        SyntaxLambdaExpr::cast(lets[3].initializer().expect("lambda").syntax().clone())
            .expect("lambda expression");
    assert_eq!(
        lambda_expr
            .param_list()
            .expect("lambda params")
            .params()
            .count(),
        1
    );
    assert_eq!(
        lambda_expr
            .body_expression()
            .expect("lambda body expression")
            .syntax()
            .kind(),
        SyntaxKind::BinaryExpr
    );

    let block_lambda = SyntaxLambdaExpr::cast(
        lets[4]
            .initializer()
            .expect("block lambda")
            .syntax()
            .clone(),
    )
    .expect("block lambda expression");
    assert_eq!(
        block_lambda
            .body_block()
            .expect("lambda block body")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ReturnStmt]
    );
}

#[test]
fn parser_parse_source_structures_match_expression_and_pattern_nodes() {
    let source = r#"fn update(state) {
    let reward = match state {
        Option::Some(value) if value > 1 => Reward { count: value },
        Quest::Active { quest_id: id, count } => {
            id
        },
        null => "empty",
        _ => "none",
    };
}
"#;
    let parse = parse_source_with_id(SourceId::new(20), source);
    let tree = parse.tree();
    let body = tree
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let let_stmt = body.let_statements().next().expect("let statement");
    let match_expr = let_stmt
        .initializer()
        .and_then(|expr| SyntaxMatchExpr::cast(expr.syntax().clone()))
        .expect("match initializer");
    let arms = match_expr
        .arm_list()
        .expect("match arm list")
        .arms()
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        match_expr.scrutinee().expect("scrutinee").syntax().kind(),
        SyntaxKind::PathExpr
    );
    assert_eq!(arms.len(), 4);

    let tuple_pattern =
        SyntaxTuplePattern::cast(arms[0].pattern().expect("tuple pattern").syntax().clone())
            .expect("tuple pattern wrapper");
    assert_eq!(tuple_pattern.path_text().as_deref(), Some("Option::Some"));
    assert_eq!(tuple_pattern.patterns().count(), 1);
    assert_eq!(
        tuple_pattern
            .patterns()
            .next()
            .expect("tuple field binding")
            .binding_name()
            .as_deref(),
        Some("value")
    );
    assert_eq!(
        arms[0]
            .expressions()
            .map(|expression| expression.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::BinaryExpr, SyntaxKind::RecordExpr]
    );
    assert_eq!(
        arms[0].guard().expect("match guard").syntax().kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(
        arms[0]
            .body_expression()
            .expect("record arm body")
            .syntax()
            .kind(),
        SyntaxKind::RecordExpr
    );
    assert!(arms[0].body_block().is_none());

    let record_pattern =
        SyntaxRecordPattern::cast(arms[1].pattern().expect("record pattern").syntax().clone())
            .expect("record pattern wrapper");
    assert_eq!(record_pattern.path_text().as_deref(), Some("Quest::Active"));
    let fields = record_pattern.fields().collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].label_text().as_deref(), Some("quest_id"));
    assert_eq!(
        fields[0]
            .pattern()
            .expect("explicit field pattern")
            .binding_name()
            .as_deref(),
        Some("id")
    );
    assert_eq!(fields[1].label_text().as_deref(), Some("count"));
    assert!(fields[1].pattern().is_none());
    assert_eq!(
        arms[1]
            .body_block()
            .expect("block body")
            .statements()
            .count(),
        1
    );
    assert!(arms[1].guard().is_none());
    assert!(arms[1].body_expression().is_none());

    assert_eq!(
        arms[2]
            .pattern()
            .expect("literal pattern")
            .literal_text()
            .as_deref(),
        Some("null")
    );
    assert_eq!(
        arms[2]
            .body_expression()
            .expect("literal arm body")
            .syntax()
            .kind(),
        SyntaxKind::Literal
    );

    assert!(arms[3].pattern().expect("wildcard pattern").is_wildcard());
    assert_eq!(
        arms[3]
            .expressions()
            .next()
            .expect("literal arm body")
            .syntax()
            .kind(),
        SyntaxKind::Literal
    );
    assert!(arms[3].guard().is_none());
    assert_eq!(
        arms[3]
            .body_expression()
            .expect("literal arm body")
            .syntax()
            .kind(),
        SyntaxKind::Literal
    );
}

#[test]
fn parser_parse_source_splits_unseparated_match_arms() {
    let source = r#"fn update(state) {
    let reward = match state {
        Quest::Done => 1
        Quest::Active { count } => count;
        Quest::Pending(value) => value
    };
}
"#;
    let parse = parse_source_with_id(SourceId::new(25), source);
    let tree = parse.tree();
    let body = tree
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let let_stmt = body.let_statements().next().expect("let statement");
    let match_expr = let_stmt
        .initializer()
        .and_then(|expr| SyntaxMatchExpr::cast(expr.syntax().clone()))
        .expect("match initializer");
    let arms = match_expr
        .arm_list()
        .expect("match arm list")
        .arms()
        .collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(arms.len(), 3);
    assert_eq!(
        arms[0]
            .pattern()
            .expect("path pattern")
            .path_text()
            .as_deref(),
        Some("Quest::Done")
    );
    assert_eq!(
        arms[0]
            .body_expression()
            .expect("literal body")
            .syntax()
            .kind(),
        SyntaxKind::Literal
    );

    let record_pattern =
        SyntaxRecordPattern::cast(arms[1].pattern().expect("record pattern").syntax().clone())
            .expect("record pattern wrapper");
    assert_eq!(record_pattern.path_text().as_deref(), Some("Quest::Active"));
    assert_eq!(record_pattern.fields().count(), 1);
    assert_eq!(
        arms[1]
            .separator_token()
            .expect("semicolon separator")
            .text(),
        ";"
    );

    let tuple_pattern =
        SyntaxTuplePattern::cast(arms[2].pattern().expect("tuple pattern").syntax().clone())
            .expect("tuple pattern wrapper");
    assert_eq!(tuple_pattern.path_text().as_deref(), Some("Quest::Pending"));
    assert_eq!(tuple_pattern.patterns().count(), 1);
}

#[test]
fn parser_parse_source_structures_for_statement_pattern_and_iterable_nodes() {
    let source = r#"fn update(rewards) {
    for index, Reward::Grant { amount: value, item } in rewards.filter(|reward| reward.active) {
        total += index + value;
    }
}
"#;
    let parse = parse_source_with_id(SourceId::new(21), source);
    let tree = parse.tree();
    let body = tree
        .functions()
        .next()
        .expect("function item")
        .body()
        .expect("function body");
    let for_stmt = body
        .syntax()
        .children()
        .find_map(SyntaxForStmt::cast)
        .expect("for statement");
    let patterns = for_stmt.patterns().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(patterns.len(), 2);
    assert_eq!(patterns[0].binding_name().as_deref(), Some("index"));

    let record_pattern =
        SyntaxRecordPattern::cast(patterns[1].syntax().clone()).expect("record pattern");
    assert_eq!(record_pattern.path_text().as_deref(), Some("Reward::Grant"));
    let fields = record_pattern.fields().collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].label_text().as_deref(), Some("amount"));
    assert_eq!(
        fields[0]
            .pattern()
            .expect("explicit field pattern")
            .binding_name()
            .as_deref(),
        Some("value")
    );
    assert_eq!(fields[1].label_text().as_deref(), Some("item"));
    assert!(fields[1].pattern().is_none());
    assert_eq!(
        for_stmt
            .iterable()
            .expect("iterable expression")
            .syntax()
            .kind(),
        SyntaxKind::CallExpr
    );
    assert_eq!(
        for_stmt
            .body()
            .expect("for body")
            .statements()
            .map(|statement| statement.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ExprStmt]
    );
}

#[test]
fn parser_parse_source_keeps_malformed_fragments_in_cst() {
    let source = "fn main() { @ \"unterminated";
    let parse = parse_source_with_id(SourceId::new(9), source);
    let tree = parse.tree();

    assert_eq!(tree.syntax().text().to_string(), source);
    assert!(
        tree.syntax()
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .any(|token| token.kind() == SyntaxKind::Unknown && token.text() == "@")
    );
    let diagnostics = parse.diagnostics();
    let diagnostic_spans = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.span)
        .collect::<Vec<_>>();
    assert!(diagnostic_spans.contains(&Span::new(SourceId::new(9), 12, 13)));
    assert!(diagnostic_spans.contains(&Span::new(SourceId::new(9), 14, source.len() as u32)));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_deref() == Some("E_PARSE") && diagnostic.message == "expected `}`"
    }));
}

#[test]
fn parser_parse_source_reports_restricted_type_hint_arguments() {
    for (source, code) in [
        (
            "fn bad(xs: Player<i64>) { return xs; }",
            "syntax::generic_type_hint",
        ),
        (
            "fn bad(xs: Map<PathProxy, String>) { return xs; }",
            "syntax::map_key_type_argument",
        ),
        (
            "fn bad(xs: Set<Function>) { return xs; }",
            "syntax::set_element_type_argument",
        ),
        (
            "fn bad(xs: Result<String>) { return xs; }",
            "syntax::type_argument_arity",
        ),
    ] {
        let parse = parse_source_with_id(SourceId::new(30), source);
        assert!(
            parse
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(code)),
            "{source}: {:?}",
            parse.diagnostics()
        );
    }
}
