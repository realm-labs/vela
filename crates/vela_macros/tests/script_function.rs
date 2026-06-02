#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_common::{FieldId, HostObjectId, HostTypeId};
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint,
};
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_macros::{script_context_function, script_function, script_host_function};
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::{VmErrorKind, VmResult};
use vela_vm::value::Value;

#[path = "script_function/metadata.rs"]
mod metadata;
#[path = "script_function/registration.rs"]
mod registration;

/// Grants a copied bonus amount.
#[script_function(
    id = 41,
    name = "game.grant_bonus",
    effect = "pure",
    reflect = true,
    permission = "bonus.read",
    attr = "domain=gameplay",
    attr = "stable=true"
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

/// Sums a copied fixed weight array.
#[script_function(id = 52, name = "game.sum_weights", effect = "pure", reflect = true)]
fn sum_weights(weights: [i64; 3]) -> i64 {
    weights.iter().sum()
}

/// Returns a copied fixed weight array.
#[script_function(
    id = 53,
    name = "game.default_weights",
    effect = "pure",
    reflect = true
)]
fn default_weights() -> [i64; 3] {
    [2, 4, 6]
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
