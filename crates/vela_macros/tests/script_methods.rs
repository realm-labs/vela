#![allow(clippy::result_large_err)]

use vela_common::{HostMethodId, HostObjectId, stable_id};
use vela_engine::engine::Engine;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, FunctionAccess, TypeHint};
use vela_engine::permission::Capability;
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::HostPath;
use vela_host::path::HostRef;
use vela_host::proxy::PathProxy;
use vela_host::resolved::{HostAccessOp, HostAccessSpec, ResolvedHostAccessKind};
use vela_host::target::HostTargetPlan;
use vela_host::value::HostValue;
use vela_macros::{ScriptHost, script_methods};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;

macro_rules! compile_source {
    ($engine:expr, $source:expr, $expect:literal) => {
        $engine.compile_source($source).expect($expect)
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
        host.access.write_diagnostic_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Scalar(vela_common::ScalarValue::I64(amount)),
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
        host.access.write_diagnostic_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Scalar(vela_common::ScalarValue::I64(total)),
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
        host.access.write_diagnostic_path(
            host.adapter,
            receiver.clone().field(Player::vela_field_id_level()),
            HostValue::Scalar(vela_common::ScalarValue::I64(total)),
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
        i64::try_from(path.to_diagnostic_path().segments.len()).expect("path depth fits i64")
    }
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectCounter")]
struct DirectCounter {
    #[script(get, set)]
    total: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectPeer")]
struct DirectPeer {
    #[script(get, set)]
    total: i64,
}

#[allow(dead_code)]
#[derive(ScriptHost)]
#[script(path = "game::counter::DirectConfig")]
struct DirectConfig {
    #[script(get)]
    bonus: i64,
}

#[script_methods]
impl DirectPeer {}

#[script_methods]
impl DirectConfig {}

#[allow(dead_code)]
#[script_methods]
impl DirectCounter {
    #[script_method(effect = "write_host", reflect = true)]
    pub fn add(&mut self, amount: i64) -> i64 {
        self.total += amount;
        self.total
    }

    #[script_method(effect = "write_host", reflect = true)]
    pub async fn add_async(&mut self, amount: i64) -> i64 {
        let mut pending = true;
        std::future::poll_fn(move |context| {
            if pending {
                pending = false;
                context.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        })
        .await;
        self.total += amount;
        self.total
    }

    #[script_method(effect = "read_host", reflect = true)]
    pub async fn read_async(&self) -> i64 {
        self.total
    }

    #[script_method(effect = "read_host")]
    pub async fn read_shared_alias(
        &self,
        context: &mut vela_engine::context::NativeCallContext<'_, '_>,
        other: &DirectCounter,
        raw: HostRef,
    ) -> VmResult<i64> {
        let mut pending = true;
        std::future::poll_fn(move |task| {
            if pending {
                pending = false;
                task.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        })
        .await;
        let _ = context
            .call_async(
                "raw_read",
                vela_engine::runtime::CallArgs::new().with_host_handle("counter", raw),
            )
            .await?;
        Ok(self.total + other.total)
    }

    #[script_method(effect = "read_host")]
    pub async fn wait_shared(&self) -> i64 {
        std::future::pending().await
    }

    #[script_method(effect = "write_host")]
    pub async fn add_with_hook(
        &mut self,
        context: &mut vela_engine::context::NativeCallContext<'_, '_>,
        raw: HostRef,
        amount: i64,
    ) -> VmResult<i64> {
        self.total += amount;
        let raw_error = context
            .call_async(
                "raw_read",
                vela_engine::runtime::CallArgs::new().with_host_handle("counter", raw),
            )
            .await
            .expect_err("the raw parent HostRef should remain busy while leased");
        if !matches!(
            raw_error.kind(),
            vela_vm::error::VmErrorKind::Host(
                vela_host::error::HostErrorKind::HostObjectBusy { .. }
            )
        ) {
            return Err(raw_error);
        }
        let _ = context
            .call_async(
                "hook",
                vela_engine::runtime::CallArgs::new().with_host_mut("counter", &mut *self),
            )
            .await?;
        self.total += 1;
        Ok(self.total)
    }

    #[script_method(effect = "write_host")]
    pub async fn update_with(
        &mut self,
        peer: &mut DirectPeer,
        config: &DirectConfig,
        amount: i64,
    ) -> i64 {
        peer.total += amount;
        self.total += peer.total + config.bonus;
        self.total
    }

    #[script_method(effect = "write_host")]
    pub async fn merge(&mut self, other: &mut DirectCounter) -> i64 {
        self.total += other.total;
        self.total
    }

    #[script_method(effect = "write_host")]
    pub async fn wait_async(&mut self) -> i64 {
        std::future::pending().await
    }

    #[script_method(effect = "write_host")]
    pub async fn wait_with_context(
        &mut self,
        _context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    ) -> i64 {
        std::future::pending().await
    }

    #[script_method(effect = "write_host")]
    pub async fn panic_with_context(
        &mut self,
        _context: &mut vela_engine::context::NativeCallContext<'_, '_>,
    ) -> i64 {
        panic!("context direct method panic fixture")
    }
}

fn method_id(name: &str) -> HostMethodId {
    HostMethodId::new(u128::from(stable_id(
        "host_method",
        "game::player::Player",
        name,
    )))
}

#[test]
fn script_methods_resolve_root_methods_to_direct_access() {
    let counter = DirectCounter { total: 1 };
    let plan = HostTargetPlan::new(DirectCounter::vela_host_type_id());
    let method = HostMethodId::new(u128::from(stable_id(
        "host_method",
        "game::counter::DirectCounter",
        "add",
    )));

    let access = <DirectCounter as vela_host::object::ScriptHostObject>::resolve_host_target(
        &counter,
        HostAccessSpec::new(HostAccessOp::Call(method), &plan),
    )
    .expect("generated script method resolver should resolve direct self method");

    assert_eq!(access.adapter_kind, ResolvedHostAccessKind::DirectMethod(0));
}
