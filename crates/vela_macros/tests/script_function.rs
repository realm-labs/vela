#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_common::{FieldId, HostObjectId, HostTypeId, SourceId};
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint,
};
use vela_engine::runtime::{CallOptions, Runtime};
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_macros::{script_context_function, script_function, script_host_function};
use vela_reflect::permissions::ReflectPermissionSet;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::{VmErrorKind, VmResult};

macro_rules! compile_source {
    ($engine:expr, $source:expr, $expect:literal) => {
        $engine
            .compile_source(SourceId::new(1), $source)
            .expect($expect)
    };
}

#[path = "script_function/metadata.rs"]
mod metadata;
#[path = "script_function/registration.rs"]
mod registration;

/// Grants a copied bonus amount.
#[script_function(
    name = "game::grant_bonus",
    effect = "pure",
    reflect = true,
    permission = "bonus.read",
    attr = "domain=gameplay",
    attr = "stable=true"
)]
fn grant_bonus(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

/// Grants a renamed copied bonus amount.
#[script_function(
    name = "game::grant_bonus_v2",
    alias = "game::grant_bonus",
    effect = "pure",
    reflect = true
)]
fn grant_bonus_v2(amount: i64) -> i64 {
    amount + 2
}

/// Sets a copied player level through PatchTx.
#[script_context_function(
    name = "game::set_level",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_level(ctx: &mut NativeCallContext<'_, '_>, player: HostRef, level: i64) -> VmResult<bool> {
    ctx.charge_instructions(3)?;
    ctx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(ctx.has_permission("player.write"))
}

/// Sets a renamed copied player level through PatchTx.
#[script_context_function(
    name = "game::set_level_v2",
    alias = "game::set_level",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_level_v2(ctx: &mut NativeCallContext<'_, '_>, player: HostRef, level: i64) -> VmResult<i64> {
    ctx.set_path(
        HostPath::new(player).field(FieldId::new(1)),
        HostValue::Int(level),
        None,
    )?;
    Ok(level)
}

/// Returns a fallible copied player level through PatchTx.
#[script_context_function(
    name = "game::checked_level",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn checked_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    level: i64,
    ok: bool,
) -> HostResult<i64> {
    let path = HostPath::new(player).field(FieldId::new(1));
    if !ok {
        return Err(HostError {
            kind: HostErrorKind::MissingPath { path },
            source_span: None,
        });
    }
    ctx.tx().set_path(path, HostValue::Int(level), None)?;
    Ok(level)
}

/// Sets a copied player score through host execution.
#[script_host_function(
    name = "game::set_score",
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

/// Sets a renamed copied player score through host execution.
#[script_host_function(
    name = "game::set_score_v2",
    alias = "game::set_score",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn set_score_v2(host: &mut HostExecution<'_>, player: HostRef, score: i64) -> VmResult<i64> {
    host.tx.set_path(
        HostPath::new(player).field(FieldId::new(2)),
        HostValue::Int(score),
        None,
    )?;
    Ok(score)
}

/// Returns a fallible copied player score through host execution.
#[script_host_function(
    name = "game::checked_score",
    effect = "write_host",
    reflect = true,
    permission = "player.write"
)]
fn checked_score(
    host: &mut HostExecution<'_>,
    player: HostRef,
    score: i64,
    ok: bool,
) -> HostResult<i64> {
    let path = HostPath::new(player).field(FieldId::new(2));
    if !ok {
        return Err(HostError {
            kind: HostErrorKind::MissingPath { path },
            source_span: None,
        });
    }
    host.tx.set_path(path, HostValue::Int(score), None)?;
    Ok(score)
}

/// Counts copied unique labels from a script set::
#[script_function(name = "game::count_labels", effect = "pure", reflect = true)]
fn count_labels(labels: BTreeSet<String>) -> i64 {
    i64::try_from(labels.len()).expect("label count fits i64")
}

/// Counts copied unordered labels from a script set::
#[script_function(name = "game::count_unordered_labels", effect = "pure", reflect = true)]
fn count_unordered_labels(labels: HashSet<String>) -> i64 {
    i64::try_from(labels.len()).expect("label count fits i64")
}

/// Sums a copied fixed weight array.
#[script_function(name = "game::sum_weights", effect = "pure", reflect = true)]
fn sum_weights(weights: [i64; 3]) -> i64 {
    weights.iter().sum()
}

/// Returns a copied fixed weight array.
#[script_function(name = "game::default_weights", effect = "pure", reflect = true)]
fn default_weights() -> [i64; 3] {
    [2, 4, 6]
}

/// Sums copied score values from a script map.
#[script_function(name = "game::score_total", effect = "pure", reflect = true)]
fn score_total(scores: HashMap<String, i64>) -> i64 {
    scores.values().sum()
}

/// Adds a copied total entry to an ordered script map.
#[script_function(name = "game::ordered_score_summary", effect = "pure", reflect = true)]
fn ordered_score_summary(mut scores: BTreeMap<String, i64>) -> BTreeMap<String, i64> {
    let total = scores.values().sum();
    scores.insert("total".to_owned(), total);
    scores
}

/// Scales a copied encounter weight.
#[script_function(name = "game::scale_weight", effect = "pure", reflect = true)]
fn scale_weight(weight: f32) -> f32 {
    weight * 1.5
}

/// Applies an optional copied bonus.
#[script_function(name = "game::optional_bonus", effect = "pure", reflect = true)]
fn optional_bonus(bonus: Option<i64>) -> Option<i64> {
    bonus.map(|bonus| bonus + 1)
}

/// Sums five copied script integers.
#[script_function(name = "game::sum5", effect = "pure", reflect = true)]
fn sum5(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    a + b + c + d + e
}

/// Sums six copied script integers.
#[script_function(name = "game::sum6", effect = "pure", reflect = true)]
fn sum6(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64) -> i64 {
    a + b + c + d + e + f
}

/// Returns a dynamic copied Result bonus.
#[script_function(name = "game::checked_bonus", effect = "pure", reflect = true)]
fn checked_bonus(ok: bool) -> std::result::Result<i64, String> {
    if ok { Ok(9) } else { Err("denied".to_owned()) }
}

/// Returns a fallible copied host bonus.
#[script_function(name = "game::checked_host_bonus", effect = "pure", reflect = true)]
fn checked_host_bonus(ok: bool) -> HostResult<i64> {
    Ok(if ok { 11 } else { 0 })
}

/// Measures a copied host path proxy.
#[script_function(name = "game::path_depth", effect = "pure", reflect = true)]
fn path_depth(path: PathProxy) -> i64 {
    i64::try_from(path.path().segments.len()).expect("path depth fits i64")
}

/// Private reflection-only debug probe.
#[script_function(
    name = "game::debug_probe",
    effect = "pure",
    public = false,
    reflect_visible = true
)]
fn debug_probe() -> bool {
    true
}

fn function_id(name: &str) -> NativeFunctionId {
    NativeFunctionId::new(vela_common::stable_id("native_function", "", name))
}
