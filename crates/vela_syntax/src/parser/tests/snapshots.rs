use super::*;

#[test]
fn snapshots_core_m1_syntax_shape() {
    let parsed = parse_source(
        source_id(),
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

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(
        snapshot_file(&parsed),
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
fn malformed_body_diagnostics_keep_source_spans() {
    let parsed = parse_source(
        source_id(),
        r#"
fn bad(player) {
    let = ;
    if player.level > {
        return;
    }
}
fn next() {}
"#,
    );

    assert!(!parsed.diagnostics.is_empty());
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.span.is_some())
    );
    assert_eq!(parsed.items.len(), 2);
    assert!(matches!(parsed.items[1].kind, ItemKind::Function(_)));
}

fn snapshot_file(file: &SourceFile) -> String {
    let mut out = String::new();
    for item in &file.items {
        match &item.kind {
            ItemKind::Use(use_item) => {
                writeln!(out, "use {}", use_item.path.join("::")).expect("write syntax snapshot");
            }
            ItemKind::Const(constant) => {
                writeln!(
                    out,
                    "const {} = {}",
                    constant.name,
                    expr_kind_name(&constant.value)
                )
                .expect("write syntax snapshot");
            }
            ItemKind::Global(global) => {
                writeln!(
                    out,
                    "global {}: {}",
                    global.name,
                    global.type_hint.path.join("::")
                )
                .expect("write syntax snapshot");
            }
            ItemKind::Function(function) => {
                let visibility = if item.visibility == Visibility::Public {
                    "pub "
                } else {
                    ""
                };
                writeln!(
                    out,
                    "{visibility}fn {}({})",
                    function.name,
                    param_names(&function.params).join(", ")
                )
                .expect("write syntax snapshot");
                snapshot_block(&mut out, &function.body, 1);
            }
            ItemKind::Struct(record) => {
                writeln!(
                    out,
                    "struct {}({})",
                    record.name,
                    struct_field_names(&record.fields).join(", ")
                )
                .expect("write syntax snapshot");
            }
            ItemKind::Enum(enumeration) => {
                writeln!(
                    out,
                    "enum {}({})",
                    enumeration.name,
                    enum_variant_names(&enumeration.variants).join(", ")
                )
                .expect("write syntax snapshot");
            }
            ItemKind::Trait(trait_item) => {
                writeln!(
                    out,
                    "trait {}({})",
                    trait_item.name,
                    trait_method_names(&trait_item.methods).join(", ")
                )
                .expect("write syntax snapshot");
            }
            ItemKind::Impl(impl_item) => {
                let methods = impl_item
                    .methods
                    .iter()
                    .map(|method| method.function.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    out,
                    "impl {} for {}({methods})",
                    impl_item.trait_path.join("::"),
                    impl_item.target_path.join("::")
                )
                .expect("write syntax snapshot");
            }
        }
    }
    out
}

fn snapshot_block(out: &mut String, block: &Block, indent: usize) {
    for stmt in &block.statements {
        snapshot_stmt(out, stmt, indent);
    }
}

fn snapshot_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    let pad = "  ".repeat(indent);
    match &stmt.kind {
        StmtKind::Let { name, value, .. } => {
            let value = value.as_ref().map_or("<none>", expr_kind_name);
            writeln!(out, "{pad}let {name} = {value}").expect("write syntax snapshot");
        }
        StmtKind::Return(value) => {
            let value = value.as_ref().map_or("<none>", expr_kind_name);
            writeln!(out, "{pad}return {value}").expect("write syntax snapshot");
        }
        StmtKind::Break => writeln!(out, "{pad}break").expect("write syntax snapshot"),
        StmtKind::Continue => writeln!(out, "{pad}continue").expect("write syntax snapshot"),
        StmtKind::For {
            index_pattern,
            pattern,
            iterable,
            body,
        } => {
            let pattern = if let Some(index_pattern) = index_pattern {
                format!(
                    "{}, {}",
                    pattern_snapshot_name(index_pattern),
                    pattern_snapshot_name(pattern)
                )
            } else {
                pattern_snapshot_name(pattern)
            };
            writeln!(out, "{pad}for {} in {}", pattern, expr_kind_name(iterable))
                .expect("write syntax snapshot");
            snapshot_block(out, body, indent + 1);
        }
        StmtKind::Expr(expr) => snapshot_expr_stmt(out, expr, indent),
        StmtKind::Block(block) => {
            writeln!(out, "{pad}block").expect("write syntax snapshot");
            snapshot_block(out, block, indent + 1);
        }
    }
}

fn snapshot_expr_stmt(out: &mut String, expr: &Expr, indent: usize) {
    let pad = "  ".repeat(indent);
    writeln!(out, "{pad}expr {}", expr_kind_name(expr)).expect("write syntax snapshot");
    match &expr.kind {
        ExprKind::If(if_expr) => snapshot_block(out, &if_expr.then_branch, indent + 1),
        ExprKind::Match(match_expr) => {
            for arm in &match_expr.arms {
                writeln!(
                    out,
                    "{pad}  arm {} => {}",
                    pattern_kind_name(&arm.pattern),
                    expr_kind_name(&arm.body)
                )
                .expect("write syntax snapshot");
            }
        }
        _ => {}
    }
}

fn expr_kind_name(expr: &Expr) -> &'static str {
    match expr.kind {
        ExprKind::Literal(_) => "literal",
        ExprKind::Path(_) => "path",
        ExprKind::SelfValue => "self",
        ExprKind::Unary { .. } => "unary",
        ExprKind::Binary { .. } => "binary",
        ExprKind::Assign { .. } => "assign",
        ExprKind::Field { .. } => "field",
        ExprKind::Call { .. } => "call",
        ExprKind::Index { .. } => "index",
        ExprKind::Try(_) => "try",
        ExprKind::Array(_) => "array",
        ExprKind::Map(_) => "map",
        ExprKind::Record { .. } => "record",
        ExprKind::Lambda { .. } => "lambda",
        ExprKind::If(_) => "if",
        ExprKind::Match(_) => "match",
        ExprKind::Block(_) => "block",
        ExprKind::Error => "error",
    }
}

fn pattern_kind_name(pattern: &Pattern) -> &'static str {
    match pattern {
        Pattern::Wildcard => "_",
        Pattern::Literal(_) => "literal",
        Pattern::Binding(_) => "binding",
        Pattern::Path(_) => "path",
        Pattern::TupleVariant { .. } => "tuple_variant",
        Pattern::RecordVariant { .. } => "record_variant",
    }
}

fn pattern_snapshot_name(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_owned(),
        Pattern::Literal(_) => "literal".to_owned(),
        Pattern::Binding(name) => name.clone(),
        Pattern::Path(path) => path.join("::"),
        Pattern::TupleVariant { path, .. } => format!("{}(...)", path.join("::")),
        Pattern::RecordVariant { path, .. } => format!("{} {{...}}", path.join("::")),
    }
}
