use vela_engine::args::FromScriptArg;
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::interop::{BoundaryMode, CallableKind, VelaValueBoundary};
use vela_engine::native::{EffectSet, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{ScriptHost, export, export_module, methods, script_methods, trait_export};
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, ScriptHost)]
#[script(path = "game::Player")]
pub struct Player {
    #[script(get, set)]
    level: i64,
}

#[trait_export(path = "game::Damageable")]
pub trait Damageable {
    fn take_damage(&mut self, amount: i64);
    fn is_alive(&self) -> bool;
}

impl Damageable for Player {
    fn take_damage(&mut self, amount: i64) {
        self.level -= amount.max(0);
    }

    fn is_alive(&self) -> bool {
        self.level > 0
    }
}

#[export_module(path = "rules")]
mod rules_exports {
    pub fn clamp(amount: i64) -> i64 {
        amount.max(0)
    }

    #[export(effects(random))]
    pub fn random_floor() -> i64 {
        1
    }

    pub(super) fn private_helper() -> i64 {
        2
    }
}

#[export(path = "game::normalize")]
pub fn normalize(amount: i64) -> i64 {
    amount.max(0)
}

#[export(path = "game::grant_exp")]
pub fn grant_exp(player: &mut Player, amount: i64) -> VmResult<()> {
    player.level += amount.max(0);
    Ok(())
}

#[export(path = "game::sum_levels")]
pub fn sum_levels(first: &Player, second: &Player) -> i64 {
    first.level + second.level
}

#[export(path = "game::transfer")]
pub fn transfer(first: &mut Player, second: &mut Player, amount: i64) -> i64 {
    first.level -= amount;
    second.level += amount;
    first.level + second.level
}

#[export(path = "game::mixed_alias")]
pub fn mixed_alias(first: &Player, second: &mut Player) -> i64 {
    second.level += first.level;
    second.level
}

pub struct StrictAmount(i64);

impl VelaValueBoundary for StrictAmount {
    fn vela_type_hint() -> TypeHint {
        TypeHint::Any
    }
}

impl FromScriptArg for StrictAmount {
    const TYPE_NAME: &'static str = "strict amount";

    fn from_script_arg(value: &OwnedValue) -> VmResult<Self> {
        match value {
            OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => Ok(Self(*value)),
            _ => Err(VmError::new(VmErrorKind::TypeMismatch {
                operation: "strict amount conversion",
            })),
        }
    }
}

#[export(path = "game::strict_grant")]
pub fn strict_grant(player: &mut Player, amount: StrictAmount) -> i64 {
    player.level += amount.0;
    player.level
}

#[export(path = "game::fail_grant")]
pub fn fail_grant(_player: &mut Player) -> VmResult<()> {
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "authored Rust failure",
    }))
}

#[export(path = "game::panic_grant")]
pub fn panic_grant(_player: &mut Player) {
    panic!("authored Rust panic payload must not cross the boundary");
}

#[export(path = "game::roll", effects(random))]
pub fn roll(_ctx: &mut NativeCallContext<'_, '_>, player: &Player) -> i64 {
    player.level
}

#[methods(path = "game::Player")]
#[script_methods]
impl Player {
    pub fn current_level(&self) -> i64 {
        self.level
    }

    pub fn increment(&mut self, amount: i64) {
        self.level += amount;
    }

    fn rust_only_helper(&self) -> i64 {
        self.level
    }
}

#[test]
fn ordinary_exports_emit_normalized_callable_contracts() {
    let normalize = vela_callable_contract_normalize();
    assert_eq!(normalize.identity.kind, CallableKind::RustFunction);
    assert_eq!(normalize.effects, EffectSet::pure());
    assert_eq!(normalize.parameters[0].mode, BoundaryMode::Value);

    let grant = vela_callable_contract_grant_exp();
    assert_eq!(grant.effects, EffectSet::host_write());
    assert_eq!(grant.parameters[0].mode, BoundaryMode::ExclusiveHost);

    let roll = vela_callable_contract_roll();
    assert_eq!(
        roll.effects,
        EffectSet::host_read().union(EffectSet::random())
    );
    assert_eq!(roll.parameters[0].mode, BoundaryMode::HiddenContext);
}

#[test]
fn method_groups_share_receiver_classification() {
    let shared = Player::vela_callable_contract_current_level();
    let exclusive = Player::vela_callable_contract_increment();

    assert_eq!(shared.effects, EffectSet::host_read());
    assert_eq!(shared.parameters[0].mode, BoundaryMode::SharedHost);
    assert_eq!(exclusive.effects, EffectSet::host_write());
    assert_eq!(exclusive.parameters[0].mode, BoundaryMode::ExclusiveHost);

    let mut player = Player { level: 3 };
    assert_eq!(player.current_level(), 3);
    player.increment(2);
    assert_eq!(player.current_level(), 5);
    assert_eq!(player.rust_only_helper(), 5);
}

#[test]
fn trait_export_uses_explicit_vela_protocol_identity() {
    let protocol = vela_protocol_contract_Damageable();

    assert_eq!(protocol.identity.public_path, "game::Damageable");
    assert_eq!(protocol.methods.len(), 2);
    assert_eq!(protocol.methods[0].effects, EffectSet::host_write());
    assert_eq!(protocol.methods[1].effects, EffectSet::host_read());

    let mut player = Player { level: 5 };
    player.take_damage(2);
    assert!(player.is_alive());
}

#[test]
fn export_module_groups_public_contracts_once() {
    let bundle = rules_exports::vela_exports();
    let contracts = bundle.contracts();

    assert_eq!(contracts.len(), 2);
    assert_eq!(contracts[0].public_path, "rules::clamp");
    assert_eq!(contracts[1].public_path, "rules::random_floor");
    assert_eq!(contracts[1].effects, EffectSet::random());
    assert_eq!(rules_exports::clamp(-2), 0);
    assert_eq!(rules_exports::random_floor(), 1);
    assert_eq!(rules_exports::private_helper(), 2);
}

#[test]
fn export_bundle_registers_value_functions_with_engine_once() {
    let engine = Engine::builder()
        .register_exports(rules_exports::vela_exports())
        .build()
        .expect("export bundle should register");
    let program = engine
        .compile_source("fn main() { return rules::clamp(-7); }")
        .expect("Vela should resolve the exported Rust function");
    let vm = engine.into_vm_for_program(program.bytecode());
    let linked = engine
        .link_compiled_program(program)
        .expect("exported Rust function should link");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        vm.run_linked_program_with_budget(&linked, "main", &[], &mut budget),
        Ok(OwnedValue::i64(0))
    );
}

fn host_export_runtime(source: &str) -> Runtime {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .capability(Capability::Random)
        .register_host_type::<Player>()
        .register_exports(vela_export_bundle_grant_exp())
        .register_exports(vela_export_bundle_sum_levels())
        .register_exports(vela_export_bundle_transfer())
        .register_exports(vela_export_bundle_mixed_alias())
        .register_exports(vela_export_bundle_roll())
        .register_exports(vela_export_bundle_strict_grant())
        .register_exports(vela_export_bundle_fail_grant())
        .register_exports(vela_export_bundle_panic_grant())
        .build()
        .expect("host exports should register");
    let program = engine
        .compile_source(source)
        .expect("host export call should compile");
    Runtime::new(engine, program).expect("host export runtime should initialize")
}

#[test]
fn host_exports_acquire_distinct_exclusive_arguments_and_write_through() {
    let mut runtime = host_export_runtime(
        "fn main(first: Player, second: Player) { return game::transfer(first, second, 3); }",
    );
    let mut first = Player { level: 10 };
    let mut second = Player { level: 4 };

    let result = runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("first", &mut first)
                .with_host_mut("second", &mut second),
            CallOptions::unbounded(),
        )
        .expect("distinct exclusive host arguments should run");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(14)));
    assert_eq!(first.level, 7);
    assert_eq!(second.level, 7);
}

#[test]
fn host_exports_allow_two_shared_aliases() {
    let mut runtime =
        host_export_runtime("fn main(player: Player) { return game::sum_levels(player, player); }");
    let player = Player { level: 6 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("player", &player),
            CallOptions::unbounded(),
        )
        .expect("shared aliases should coexist");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
}

#[test]
fn host_exports_reject_mixed_aliases_before_authored_rust_runs() {
    let mut runtime = host_export_runtime(
        "fn main(player: Player) { return game::mixed_alias(player, player); }",
    );
    let mut player = Player { level: 6 };

    let error = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect_err("shared plus exclusive alias must fail before the Rust body");

    assert_eq!(
        error.kind(),
        VmErrorKind::AliasedMutableHostArguments {
            callable: "game::mixed_alias".to_owned(),
            first_parameter: "first".to_owned(),
            second_parameter: "second".to_owned(),
        }
    );
    assert_eq!(player.level, 6);
}

#[test]
fn context_host_exports_receive_hidden_context_and_shared_host_reference() {
    let mut runtime = host_export_runtime("fn main(player: Player) { return game::roll(player); }");
    let player = Player { level: 9 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("player", &player),
            CallOptions::unbounded(),
        )
        .expect("context host export should run through the same lease adapter");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(9)));
}

#[test]
fn host_export_releases_acquired_lease_when_later_value_conversion_fails() {
    let mut runtime = host_export_runtime(
        "fn main(player: Player, amount) { return game::strict_grant(player, amount); }",
    );
    let mut player = Player { level: 5 };

    let error = runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("player", &mut player)
                .with_value("amount", "bad"),
            CallOptions::unbounded(),
        )
        .expect_err("bad trailing value should fail conversion");
    assert!(
        matches!(error.kind(), VmErrorKind::TypeMismatch { .. }),
        "unexpected conversion error: {:?}",
        error.kind()
    );
    assert_eq!(player.level, 5);

    let result = runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("player", &mut player)
                .with_value("amount", 2_i64),
            CallOptions::unbounded(),
        )
        .expect("the failed conversion must have released the exclusive lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    assert_eq!(player.level, 7);
}

#[test]
fn host_export_releases_exclusive_lease_on_authored_error() {
    let mut runtime = host_export_runtime("fn main(player: Player) { game::fail_grant(player); }");
    let mut player = Player { level: 5 };

    let error = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect_err("authored VmResult error should cross the boundary");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "authored Rust failure"
        }
    );

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect_err("the authored failure is repeatable after lease cleanup");
    assert!(matches!(result.kind(), VmErrorKind::TypeMismatch { .. }));
}

#[test]
fn host_export_converts_panic_and_releases_exclusive_lease() {
    let mut runtime = host_export_runtime("fn main(player: Player) { game::panic_grant(player); }");
    let mut player = Player { level: 5 };

    for _ in 0..2 {
        let error = runtime
            .call(
                "main",
                CallArgs::new().with_host_mut("player", &mut player),
                CallOptions::unbounded(),
            )
            .expect_err("Rust panic should become a stable VM error");
        assert_eq!(
            error.kind(),
            VmErrorKind::RustCallablePanicked {
                callable: "game::panic_grant".to_owned(),
            }
        );
    }
    assert_eq!(player.level, 5);
}
