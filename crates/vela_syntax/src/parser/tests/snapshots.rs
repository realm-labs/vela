use std::fmt::Write as _;

use crate::SyntaxKind;
use crate::ast::{
    AstNode, SyntaxBlock, SyntaxConstItem, SyntaxEnumItem, SyntaxExpression, SyntaxExpressionKind,
    SyntaxFunctionItem, SyntaxGlobalItem, SyntaxImplItem, SyntaxPattern, SyntaxPatternKind,
    SyntaxSourceFile, SyntaxStatement, SyntaxStatementKind, SyntaxStructItem, SyntaxTraitItem,
    SyntaxUseItem,
};
use crate::parse::{Parse, parse_source_with_id};

use super::source_id;

fn parse_cst(text: &str) -> Parse<SyntaxSourceFile> {
    parse_source_with_id(source_id(), text)
}

#[test]
fn snapshots_core_m1_syntax_shape() {
    let parsed = parse_cst(
        r#"
use game::player::Player;

const START_LEVEL = 1 + 2;

#[event("monster.kill")]
pub fn on_kill(ctx, player, monster) {
    let rewards = ctx.config.kill_rewards.filter(|r| r.monster_id == monster.id);
    player.exp += monster.exp;
    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1;
    }
    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count);
    }
    match player.quest_progress {
        QuestProgress::Active { quest_id, count } => {
            player.quest_progress = QuestProgress::Active { quest_id, count: count + 1 };
        },
        _ => {},
    }
}

struct KillReward { item_id, count }
enum QuestProgress { None, Active { quest_id, count } }
trait Damageable { fn damage(self, amount); }
impl Damageable for Player { fn damage(self, amount) { return amount; } }
"#,
    );

    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(
        snapshot_file(&parsed.tree()),
        r#"use game::player::Player
const START_LEVEL = binary
pub fn on_kill(ctx, player, monster)
  let rewards = call
  expr assign
  expr if
    expr assign
  for reward in path
    expr call
  expr match
    arm record_variant => block
    arm _ => block
struct KillReward(item_id, count)
enum QuestProgress(None, Active)
trait Damageable(damage)
impl Damageable for Player(damage)
"#
    );
}

#[test]
fn malformed_item_diagnostics_keep_source_spans() {
    let parsed = parse_cst(
        r#"
fn () {
    return;
}
fn next() {}
"#,
    );

    assert!(!parsed.diagnostics().is_empty());
    assert!(
        parsed
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.span.is_some())
    );
    let items = parsed.tree().items().collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].syntax().kind(), SyntaxKind::FunctionItem);
}

fn snapshot_file(file: &SyntaxSourceFile) -> String {
    let mut out = String::new();
    for item in file.items() {
        match item.syntax().kind() {
            SyntaxKind::UseItem => {
                let use_item = SyntaxUseItem::cast(item.syntax().clone()).expect("use item");
                writeln!(
                    out,
                    "use {}",
                    use_item
                        .path()
                        .expect("use path")
                        .path_segments()
                        .join("::")
                )
                .expect("write syntax snapshot");
            }
            SyntaxKind::ConstItem => {
                let constant = SyntaxConstItem::cast(item.syntax().clone()).expect("const item");
                writeln!(
                    out,
                    "const {} = {}",
                    required_text(constant.name_text()),
                    expr_kind_name(&constant.value().expect("const value"))
                )
                .expect("write syntax snapshot");
            }
            SyntaxKind::GlobalItem => {
                let global = SyntaxGlobalItem::cast(item.syntax().clone()).expect("global item");
                writeln!(
                    out,
                    "global {}: {}",
                    required_text(global.name_text()),
                    global
                        .type_hint()
                        .expect("global type")
                        .path_segments()
                        .join("::")
                )
                .expect("write syntax snapshot");
            }
            SyntaxKind::FunctionItem => {
                let function =
                    SyntaxFunctionItem::cast(item.syntax().clone()).expect("function item");
                let visibility = if item.is_public() { "pub " } else { "" };
                writeln!(
                    out,
                    "{visibility}fn {}({})",
                    required_text(function.name_text()),
                    function_param_names(&function).join(", ")
                )
                .expect("write syntax snapshot");
                snapshot_block(&mut out, &function.body().expect("function body"), 1);
            }
            SyntaxKind::StructItem => {
                let record = SyntaxStructItem::cast(item.syntax().clone()).expect("struct item");
                writeln!(
                    out,
                    "struct {}({})",
                    required_text(record.name_text()),
                    record_field_names(&record).join(", ")
                )
                .expect("write syntax snapshot");
            }
            SyntaxKind::EnumItem => {
                let enumeration = SyntaxEnumItem::cast(item.syntax().clone()).expect("enum item");
                writeln!(
                    out,
                    "enum {}({})",
                    required_text(enumeration.name_text()),
                    variant_names(&enumeration).join(", ")
                )
                .expect("write syntax snapshot");
            }
            SyntaxKind::TraitItem => {
                let trait_item = SyntaxTraitItem::cast(item.syntax().clone()).expect("trait item");
                writeln!(
                    out,
                    "trait {}({})",
                    required_text(trait_item.name_text()),
                    trait_method_names(&trait_item).join(", ")
                )
                .expect("write syntax snapshot");
            }
            SyntaxKind::ImplItem => {
                let impl_item = SyntaxImplItem::cast(item.syntax().clone()).expect("impl item");
                let methods = impl_item
                    .methods()
                    .map(|method| required_text(method.name_text()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let target_path = impl_item.target_path_segments().join("::");
                let trait_path = impl_item.trait_path_segments();
                if trait_path.is_empty() {
                    writeln!(out, "impl {target_path}({methods})")
                } else {
                    writeln!(
                        out,
                        "impl {} for {target_path}({methods})",
                        trait_path.join("::")
                    )
                }
                .expect("write syntax snapshot");
            }
            kind => panic!("unexpected item kind in snapshot: {kind:?}"),
        }
    }
    out
}

fn snapshot_block(out: &mut String, block: &SyntaxBlock, indent: usize) {
    for stmt in block.statements() {
        snapshot_stmt(out, &stmt, indent);
    }
}

fn snapshot_stmt(out: &mut String, stmt: &SyntaxStatement, indent: usize) {
    let pad = "  ".repeat(indent);
    match stmt.statement_kind() {
        SyntaxStatementKind::Let => {
            let stmt = stmt.as_let().expect("let statement");
            let value = stmt.initializer().as_ref().map_or("<none>", expr_kind_name);
            writeln!(
                out,
                "{pad}let {} = {value}",
                required_text(stmt.name_text())
            )
            .expect("write syntax snapshot");
        }
        SyntaxStatementKind::Return => {
            let stmt = stmt.as_return().expect("return statement");
            let value = stmt.expression().as_ref().map_or("<none>", expr_kind_name);
            writeln!(out, "{pad}return {value}").expect("write syntax snapshot");
        }
        SyntaxStatementKind::Break => {
            writeln!(out, "{pad}break").expect("write syntax snapshot");
        }
        SyntaxStatementKind::Continue => {
            writeln!(out, "{pad}continue").expect("write syntax snapshot");
        }
        SyntaxStatementKind::For => {
            let stmt = stmt.as_for().expect("for statement");
            let pattern = if let Some(index_pattern) = stmt.index_pattern() {
                format!(
                    "{}, {}",
                    pattern_snapshot_name(&index_pattern),
                    pattern_snapshot_name(&stmt.value_pattern().expect("for value pattern"))
                )
            } else {
                pattern_snapshot_name(&stmt.value_pattern().expect("for value pattern"))
            };
            writeln!(
                out,
                "{pad}for {} in {}",
                pattern,
                expr_kind_name(&stmt.iterable().expect("for iterable"))
            )
            .expect("write syntax snapshot");
            snapshot_block(out, &stmt.body().expect("for body"), indent + 1);
        }
        SyntaxStatementKind::If | SyntaxStatementKind::Match => {
            let expr = SyntaxExpression::cast(stmt.syntax().clone()).expect("statement expr");
            snapshot_expr_stmt(out, &expr, indent);
        }
        SyntaxStatementKind::Expr => {
            let expr = stmt
                .as_expr()
                .expect("expression statement")
                .expression()
                .expect("expression");
            snapshot_expr_stmt(out, &expr, indent);
        }
        SyntaxStatementKind::Block => {
            writeln!(out, "{pad}block").expect("write syntax snapshot");
            snapshot_block(out, &stmt.as_block().expect("block statement"), indent + 1);
        }
    }
}

fn snapshot_expr_stmt(out: &mut String, expr: &SyntaxExpression, indent: usize) {
    let pad = "  ".repeat(indent);
    writeln!(out, "{pad}expr {}", expr_kind_name(expr)).expect("write syntax snapshot");
    match expr.expression_kind() {
        SyntaxExpressionKind::If => {
            let if_expr = expr.as_if().expect("if expression");
            snapshot_block(out, &if_expr.then_block().expect("then block"), indent + 1);
        }
        SyntaxExpressionKind::Match => {
            let match_expr = expr.as_match().expect("match expression");
            for arm in match_expr.arms() {
                writeln!(
                    out,
                    "{pad}  arm {} => {}",
                    pattern_kind_name(&arm.pattern().expect("arm pattern")),
                    expr_kind_name(&arm.body_as_expression().expect("arm body"))
                )
                .expect("write syntax snapshot");
            }
        }
        _ => {}
    }
}

fn expr_kind_name(expr: &SyntaxExpression) -> &'static str {
    match expr.expression_kind() {
        SyntaxExpressionKind::Literal => "literal",
        SyntaxExpressionKind::Path => "path",
        SyntaxExpressionKind::Paren => "paren",
        SyntaxExpressionKind::Unary => "unary",
        SyntaxExpressionKind::Binary => "binary",
        SyntaxExpressionKind::Assign => "assign",
        SyntaxExpressionKind::Field => "field",
        SyntaxExpressionKind::Call => "call",
        SyntaxExpressionKind::Index => "index",
        SyntaxExpressionKind::Try => "try",
        SyntaxExpressionKind::Array => "array",
        SyntaxExpressionKind::Map => "map",
        SyntaxExpressionKind::Record => "record",
        SyntaxExpressionKind::Lambda => "lambda",
        SyntaxExpressionKind::Block => "block",
        SyntaxExpressionKind::If => "if",
        SyntaxExpressionKind::Match => "match",
    }
}

fn pattern_kind_name(pattern: &SyntaxPattern) -> &'static str {
    match pattern.pattern_kind().expect("pattern kind") {
        SyntaxPatternKind::Wildcard => "_",
        SyntaxPatternKind::Literal => "literal",
        SyntaxPatternKind::Binding => "binding",
        SyntaxPatternKind::Path => "path",
        SyntaxPatternKind::TupleVariant => "tuple_variant",
        SyntaxPatternKind::RecordVariant => "record_variant",
    }
}

fn pattern_snapshot_name(pattern: &SyntaxPattern) -> String {
    match pattern.pattern_kind().expect("pattern kind") {
        SyntaxPatternKind::Wildcard => "_".to_owned(),
        SyntaxPatternKind::Literal => "literal".to_owned(),
        SyntaxPatternKind::Binding => required_text(pattern.binding_name()),
        SyntaxPatternKind::Path => pattern.path_segments().join("::"),
        SyntaxPatternKind::TupleVariant => format!(
            "{}(...)",
            pattern
                .tuple_pattern()
                .expect("tuple pattern")
                .path_segments()
                .join("::")
        ),
        SyntaxPatternKind::RecordVariant => format!(
            "{} {{...}}",
            pattern
                .record_pattern()
                .expect("record pattern")
                .path_segments()
                .join("::")
        ),
    }
}

fn function_param_names(function: &SyntaxFunctionItem) -> Vec<String> {
    function
        .param_list()
        .expect("function params")
        .params()
        .map(|param| required_text(param.name_text()))
        .collect()
}

fn record_field_names(record: &SyntaxStructItem) -> Vec<String> {
    record
        .field_list()
        .expect("struct fields")
        .fields()
        .map(|field| required_text(field.name_text()))
        .collect()
}

fn variant_names(enumeration: &SyntaxEnumItem) -> Vec<String> {
    enumeration
        .variant_list()
        .expect("enum variants")
        .variants()
        .map(|variant| required_text(variant.name_text()))
        .collect()
}

fn trait_method_names(trait_item: &SyntaxTraitItem) -> Vec<String> {
    trait_item
        .methods()
        .map(|method| required_text(method.name_text()))
        .collect()
}

fn required_text(text: Option<String>) -> String {
    text.expect("syntax node text")
}
