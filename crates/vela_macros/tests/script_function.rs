#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_common::{FieldId, HostObjectId, HostTypeId};
use vela_engine::{
    EffectSet, Engine, FunctionAccess, HostRef, NativeCallContext, NativeFunctionDesc,
    NativeFunctionId, TypeHint, Value,
};
use vela_host::{HostPath, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_macros::{script_context_function, script_function, script_host_function};
use vela_vm::{HostExecution, VmResult};

/// Grants a copied bonus amount.
#[script_function(
    id = 41,
    name = "game.grant_bonus",
    effect = "pure",
    reflect = true,
    permission = "bonus.read"
)]
fn grant_bonus(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

/// Sets a copied player level through PatchTx.
#[script_context_function(
    id = 42,
    name = "game.set_level",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_level(ctx: &mut NativeCallContext<'_, '_>, player: HostRef, level: i64) -> VmResult<bool> {
    ctx.charge_instructions(3)?;
    ctx.tx().set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(ctx.has_permission("player.write"))
}

/// Sets a copied player score through host execution.
#[script_host_function(
    id = 43,
    name = "game.set_score",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_score(host: &mut HostExecution<'_>, player: HostRef, score: i64) -> VmResult<i64> {
    host.tx.set_path(
        HostPath::new(player).field(FieldId::new(2)),
        HostValue::Int(score),
        None,
    )?;
    Ok(score)
}

/// Counts copied unique labels from a script set.
#[script_function(id = 44, name = "game.count_labels", effect = "pure", reflect = true)]
fn count_labels(labels: BTreeSet<String>) -> i64 {
    i64::try_from(labels.len()).expect("label count fits i64")
}

/// Counts copied unordered labels from a script set.
#[script_function(
    id = 51,
    name = "game.count_unordered_labels",
    effect = "pure",
    reflect = true
)]
fn count_unordered_labels(labels: HashSet<String>) -> i64 {
    i64::try_from(labels.len()).expect("label count fits i64")
}

/// Sums copied score values from a script map.
#[script_function(id = 45, name = "game.score_total", effect = "pure", reflect = true)]
fn score_total(scores: HashMap<String, i64>) -> i64 {
    scores.values().sum()
}

/// Adds a copied total entry to an ordered script map.
#[script_function(
    id = 50,
    name = "game.ordered_score_summary",
    effect = "pure",
    reflect = true
)]
fn ordered_score_summary(mut scores: BTreeMap<String, i64>) -> BTreeMap<String, i64> {
    let total = scores.values().sum();
    scores.insert("total".to_owned(), total);
    scores
}

/// Scales a copied encounter weight.
#[script_function(id = 46, name = "game.scale_weight", effect = "pure", reflect = true)]
fn scale_weight(weight: f32) -> f32 {
    weight * 1.5
}

/// Applies an optional copied bonus.
#[script_function(id = 47, name = "game.optional_bonus", effect = "pure", reflect = true)]
fn optional_bonus(bonus: Option<i64>) -> Option<i64> {
    bonus.map(|bonus| bonus + 1)
}

/// Sums five copied script integers.
#[script_function(id = 48, name = "game.sum5", effect = "pure", reflect = true)]
fn sum5(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    a + b + c + d + e
}

/// Sums six copied script integers.
#[script_function(id = 50, name = "game.sum6", effect = "pure", reflect = true)]
fn sum6(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64) -> i64 {
    a + b + c + d + e + f
}

/// Returns a dynamic copied Result bonus.
#[script_function(id = 49, name = "game.checked_bonus", effect = "pure", reflect = true)]
fn checked_bonus(ok: bool) -> std::result::Result<i64, String> {
    if ok { Ok(9) } else { Err("denied".to_owned()) }
}

#[test]
fn script_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_grant_bonus(),
        NativeFunctionDesc::new("game.grant_bonus", NativeFunctionId::new(41))
            .param("amount", TypeHint::Int)
            .param("multiplier", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("bonus.read"),
            )
            .docs("Grants a copied bonus amount."),
    );
}

#[test]
fn script_function_generates_set_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_count_labels(),
        NativeFunctionDesc::new("game.count_labels", NativeFunctionId::new(44))
            .param("labels", TypeHint::Set)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Counts copied unique labels from a script set."),
    );
}

#[test]
fn script_function_generates_hash_set_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_count_unordered_labels(),
        NativeFunctionDesc::new("game.count_unordered_labels", NativeFunctionId::new(51))
            .param("labels", TypeHint::Set)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Counts copied unordered labels from a script set."),
    );
}

#[test]
fn script_function_generates_hash_map_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_score_total(),
        NativeFunctionDesc::new("game.score_total", NativeFunctionId::new(45))
            .param("scores", TypeHint::Map)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Sums copied score values from a script map."),
    );
}

#[test]
fn script_function_generates_btree_map_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_ordered_score_summary(),
        NativeFunctionDesc::new("game.ordered_score_summary", NativeFunctionId::new(50))
            .param("scores", TypeHint::Map)
            .returns(TypeHint::Map)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Adds a copied total entry to an ordered script map."),
    );
}

#[test]
fn script_function_generates_f32_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_scale_weight(),
        NativeFunctionDesc::new("game.scale_weight", NativeFunctionId::new(46))
            .param("weight", TypeHint::Float)
            .returns(TypeHint::Float)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Scales a copied encounter weight."),
    );
}

#[test]
fn script_function_generates_option_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_optional_bonus(),
        NativeFunctionDesc::new("game.optional_bonus", NativeFunctionId::new(47))
            .param("bonus", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Applies an optional copied bonus."),
    );
}

#[test]
fn script_function_generates_five_arg_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_sum5(),
        NativeFunctionDesc::new("game.sum5", NativeFunctionId::new(48))
            .param("a", TypeHint::Int)
            .param("b", TypeHint::Int)
            .param("c", TypeHint::Int)
            .param("d", TypeHint::Int)
            .param("e", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Sums five copied script integers."),
    );
}

#[test]
fn script_function_generates_result_signature_metadata() {
    assert_eq!(
        vela_native_function_desc_checked_bonus(),
        NativeFunctionDesc::new("game.checked_bonus", NativeFunctionId::new(49))
            .param("ok", TypeHint::Bool)
            .returns(TypeHint::Any)
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true))
            .docs("Returns a dynamic copied Result bonus."),
    );
}

#[test]
fn script_context_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_set_level(),
        NativeFunctionDesc::new("game.set_level", NativeFunctionId::new(42))
            .param("player", TypeHint::Any)
            .param("level", TypeHint::Int)
            .returns(TypeHint::Bool)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a copied player level through PatchTx."),
    );
}

#[test]
fn script_host_function_generates_native_function_metadata() {
    assert_eq!(
        vela_native_function_desc_set_score(),
        NativeFunctionDesc::new("game.set_score", NativeFunctionId::new(43))
            .param("player", TypeHint::Any)
            .param("score", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::host_write())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission("player.write"),
            )
            .docs("Sets a copied player score through host execution."),
    );
}

#[test]
fn script_function_registers_typed_native_with_engine() {
    let engine =
        vela_register_native_function_grant_bonus(Engine::builder().grant_permission("bonus.read"))
            .build()
            .expect("engine should build from macro native function");
    let root = unique_test_dir("script_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.grant_bonus(6, 7);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(42)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_set_native_with_engine() {
    let engine = vela_register_native_function_count_labels(Engine::builder())
        .build()
        .expect("engine should build from macro set native function");
    let root = unique_test_dir("script_function_set_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(labels) {
    return game.count_labels(labels);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered set native");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Set(vec![
                Value::String("raid".to_owned()),
                Value::String("pvp".to_owned()),
                Value::String("raid".to_owned()),
            ])],
        ),
        Ok(Value::Int(2)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_hash_set_native_with_engine() {
    let engine = vela_register_native_function_count_unordered_labels(Engine::builder())
        .build()
        .expect("engine should build from macro unordered set native function");
    let root = unique_test_dir("script_function_hash_set_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(labels) {
    return game.count_unordered_labels(labels);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered unordered set native");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Set(vec![
                Value::String("raid".to_owned()),
                Value::String("pvp".to_owned()),
                Value::String("raid".to_owned()),
            ])],
        ),
        Ok(Value::Int(2)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_hash_map_native_with_engine() {
    let engine = vela_register_native_function_score_total(Engine::builder())
        .build()
        .expect("engine should build from macro map native function");
    let root = unique_test_dir("script_function_hash_map_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(scores) {
    return game.score_total(scores);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered map native");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Map(
                [
                    ("daily".to_owned(), Value::Int(3)),
                    ("weekly".to_owned(), Value::Int(7)),
                ]
                .into(),
            )],
        ),
        Ok(Value::Int(10)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_btree_map_native_with_engine() {
    let engine = vela_register_native_function_ordered_score_summary(Engine::builder())
        .build()
        .expect("engine should build from macro ordered map native function");
    let root = unique_test_dir("script_function_btree_map_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(scores) {
    let summary = game.ordered_score_summary(scores);
    return summary.get_or("total", 0) + summary.get_or("daily", 0);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered ordered map native");

    assert_eq!(
        engine.into_vm().run_program(
            &program,
            "main",
            &[Value::Map(
                [
                    ("daily".to_owned(), Value::Int(3)),
                    ("weekly".to_owned(), Value::Int(7)),
                ]
                .into(),
            )],
        ),
        Ok(Value::Int(13)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_f32_native_with_engine() {
    let engine = vela_register_native_function_scale_weight(Engine::builder())
        .build()
        .expect("engine should build from macro f32 native function");
    let root = unique_test_dir("script_function_f32_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.scale_weight(2.0);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered f32 native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Float(3.0)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_option_native_with_engine() {
    let engine = vela_register_native_function_optional_bonus(Engine::builder())
        .build()
        .expect("engine should build from macro option native function");
    let root = unique_test_dir("script_function_option_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.optional_bonus(null) == null && game.optional_bonus(4) == 5;
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered option native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Bool(true)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_five_arg_native_with_engine() {
    let engine = vela_register_native_function_sum5(Engine::builder())
        .build()
        .expect("engine should build from macro five-arg native function");
    let root = unique_test_dir("script_function_five_arg_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.sum5(1, 2, 3, 4, 5);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered five-arg native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(15)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_six_arg_native_with_engine() {
    let engine = vela_register_native_function_sum6(Engine::builder())
        .build()
        .expect("engine should build from macro six-arg native function");
    let root = unique_test_dir("script_function_six_arg_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    return game.sum6(1, 2, 3, 4, 5, 6);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered six-arg native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(21)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_function_registers_typed_result_native_with_engine() {
    let engine =
        vela_register_native_function_checked_bonus(Engine::builder().with_standard_natives())
            .build()
            .expect("engine should build from macro result native function");
    let root = unique_test_dir("script_function_result_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main() {
    let ok = game.checked_bonus(true);
    let err = game.checked_bonus(false);
    return result.unwrap_or(ok, 0) + result.unwrap_or(err, 4);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered result native");

    assert_eq!(
        engine.into_vm().run_program(&program, "main", &[]),
        Ok(Value::Int(13)),
    );
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_context_function_registers_typed_native_with_engine() {
    let engine = vela_register_context_native_function_set_level(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro context native function");
    let root = unique_test_dir("script_context_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(player) {
    return game.set_level(player, 9);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered context native");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Bool(true)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(9)));
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

#[test]
fn script_host_function_registers_typed_native_with_engine() {
    let engine = vela_register_host_native_function_set_score(
        Engine::builder().grant_permission("player.write"),
    )
    .build()
    .expect("engine should build from macro host native function");
    let root = unique_test_dir("script_host_function_native");
    std::fs::create_dir_all(&root).expect("create temp source dir");
    let source = root.join("main.lang");
    std::fs::write(
        &source,
        r#"
fn main(player) {
    return game.set_score(player, 12);
}
"#,
    )
    .expect("write source");
    let program = engine
        .compile_file(&source)
        .expect("source should compile with macro registered host native");
    let player = HostRef::new(HostTypeId::new(1001), HostObjectId::new(42), 1);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        tx: &mut tx,
    };

    assert_eq!(
        engine.into_vm().run_program_with_host(
            &program,
            "main",
            &[Value::HostRef(player)],
            &mut host,
        ),
        Ok(Value::Int(12)),
    );
    assert_eq!(tx.patches()[0].op, PatchOp::Set(HostValue::Int(12)));
    std::fs::remove_dir_all(root).expect("clean temp source dir");
}

fn unique_test_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_macros_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    path
}
