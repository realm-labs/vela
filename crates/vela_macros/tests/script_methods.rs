#![allow(clippy::result_large_err)]

use vela_common::{HostMethodId, HostObjectId, SourceId, stable_id};
use vela_engine::engine::Engine;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, FunctionAccess, TypeHint};
use vela_engine::permission::Capability;
use vela_host::mock::MockStateAdapter;
use vela_host::path::HostPath;
use vela_host::path::HostRef;
use vela_host::proxy::PathProxy;
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;

macro_rules! compile_source {
    ($engine:expr, $source:expr, $expect:literal) => {
        $engine
            .compile_source(SourceId::new(1), $source)
            .expect($expect)
    };
}

#[path = "script_methods/metadata.rs"]
mod metadata;
#[path = "script_methods/registration.rs"]
mod registration;

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::player::Player")]
struct Player {
    #[script(get, set)]
    level: u32,
}

#[allow(dead_code)]
#[script_methods]
impl Player {
    /// Grants copied experience through the host patch path.
    #[script_method(effect = "write_host", reflect = true, attr = "domain=player")]
    pub fn grant_exp(
        _ctx: &mut vela_engine::context::NativeCallContext<'_, '_>,
        _player: HostRef,
        _amount: i64,
    ) {
    }

    /// Grants copied score through a callable native method.
    #[script_method(effect = "write_host", reflect = true)]
    pub fn grant_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        amount: i64,
    ) -> VmResult<i64> {
        host.tx.set_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Int(amount),
            None,
        )?;
        Ok(amount)
    }

    /// Previews an optional copied bonus through a callable native method.
    #[script_method(effect = "read_host", reflect = true)]
    pub fn preview_bonus(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        bonus: Option<i64>,
    ) -> Option<i64> {
        bonus.map(|bonus| bonus + 1)
    }

    /// Sums five copied method values through a callable native method.
    #[script_method(effect = "write_host", reflect = true)]
    pub fn sum_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
    ) -> VmResult<i64> {
        let total = a + b + c + d + e;
        host.tx.set_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Int(total),
            None,
        )?;
        Ok(total)
    }

    /// Sums six copied method values through a callable native method.
    #[allow(clippy::too_many_arguments)]
    #[script_method(effect = "write_host", reflect = true)]
    pub fn sum6_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
        f: i64,
    ) -> VmResult<i64> {
        let total = a + b + c + d + e + f;
        host.tx.set_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Int(total),
            None,
        )?;
        Ok(total)
    }

    /// Previews a dynamic copied Result through a callable native method.
    #[script_method(effect = "read_host", reflect = true)]
    pub fn checked_preview(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        ok: bool,
    ) -> std::result::Result<i64, String> {
        if ok {
            Ok(17)
        } else {
            Err("blocked".to_owned())
        }
    }

    /// Measures an extra copied path proxy argument.
    #[script_method(effect = "read_host", reflect = true)]
    pub fn inspect_path(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        path: PathProxy,
    ) -> i64 {
        i64::try_from(path.path().segments.len()).expect("path depth fits i64")
    }
}

fn method_id(name: &str) -> HostMethodId {
    HostMethodId::new(stable_id("host_method", "game::player::Player", name))
}
