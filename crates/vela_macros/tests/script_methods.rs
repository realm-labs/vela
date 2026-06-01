#![allow(clippy::result_large_err)]

use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, TypeId};
use vela_engine::{EffectSet, Engine, FunctionAccess, HostRef, NativeMethodDesc, TypeHint, Value};
use vela_host::{HostPath, HostValue, MockStateAdapter, PatchOp, PatchTx};
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::{HostExecution, VmResult};

#[path = "script_methods/metadata.rs"]
mod metadata;
#[path = "script_methods/registration.rs"]
mod registration;

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(id = 1001, name = "Player")]
struct Player {
    #[script(get, set, id = 1)]
    level: u32,
}

#[allow(dead_code)]
#[script_methods]
impl Player {
    /// Grants copied experience through the host patch path.
    #[script_method(
        id = 7,
        effect = "write_host",
        permission = "player.write",
        reflect = true,
        attr = "domain=player"
    )]
    pub fn grant_exp(
        _ctx: &mut vela_engine::NativeCallContext<'_, '_>,
        _player: HostRef,
        _amount: i64,
    ) {
    }

    /// Grants copied score through a callable native method.
    #[script_method(
        id = 8,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
    pub fn grant_score(
        receiver: &HostPath,
        host: &mut HostExecution<'_>,
        amount: i64,
    ) -> VmResult<i64> {
        host.tx.set_path(
            receiver.clone().field(FieldId::new(1)),
            HostValue::Int(amount),
            None,
        )?;
        Ok(amount)
    }

    /// Previews an optional copied bonus through a callable native method.
    #[script_method(id = 9, effect = "read_host", reflect = true)]
    pub fn preview_bonus(
        _receiver: &HostPath,
        _host: &mut HostExecution<'_>,
        bonus: Option<i64>,
    ) -> Option<i64> {
        bonus.map(|bonus| bonus + 1)
    }

    /// Sums five copied method values through a callable native method.
    #[script_method(
        id = 10,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
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
            receiver.clone().field(FieldId::new(1)),
            HostValue::Int(total),
            None,
        )?;
        Ok(total)
    }

    /// Sums six copied method values through a callable native method.
    #[allow(clippy::too_many_arguments)]
    #[script_method(
        id = 12,
        effect = "write_host",
        permission = "player.write",
        reflect = true
    )]
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
            receiver.clone().field(FieldId::new(1)),
            HostValue::Int(total),
            None,
        )?;
        Ok(total)
    }

    /// Previews a dynamic copied Result through a callable native method.
    #[script_method(id = 11, effect = "read_host", reflect = true)]
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
