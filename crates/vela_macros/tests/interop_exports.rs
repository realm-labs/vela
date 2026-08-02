use vela_bytecode::UnlinkedInstructionKind;
use vela_bytecode::compiler::error::CompileErrorKind;
use vela_engine::args::FromScriptArg;
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::interop::{BoundaryMode, CallableKind, VelaValueBoundary};
use vela_engine::native::{EffectSet, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::registration::{TypeRegistration, VelaBindings};
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::source::EngineSourceErrorKind;
use vela_host::mock::MockStateAdapter;
use vela_host::path::HostRef;
use vela_host::value::HostValue;
use vela_macros::{
    ScriptHost, export, export_external_trait_impl, export_module, external_host,
    external_value_enum, methods, trait_export,
};
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, ScriptHost)]
#[vela(path = "game::Player")]
pub struct Player {
    #[vela(get, set)]
    level: i64,
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::PlayerService")]
pub struct PlayerService {
    player: Player,
    #[vela(get, set)]
    touches: i64,
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::Team")]
pub struct Team {
    first: Player,
    second: Player,
    #[vela(get, set)]
    marker: i64,
}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::FieldOnly")]
pub struct FieldOnly {
    #[vela(get, hint = "FieldChild")]
    child: FieldChild,
}

#[methods(path = "game::FieldOnly")]
impl FieldOnly {}

#[derive(Debug, ScriptHost)]
#[vela(path = "game::FieldChild")]
pub struct FieldChild {
    #[vela(get)]
    value: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExternalQuality {
    Normal,
    Rare,
}

external_value_enum! {
    path = "generated::ExternalQuality",
    ty = ExternalQuality,
    variants = [Normal, Rare],
}

#[derive(Debug)]
struct ExternalConfig {
    item: ExternalItem,
}

impl ExternalConfig {
    fn item(&self) -> &ExternalItem {
        &self.item
    }
}

#[external_host(
    path = "generated::ExternalConfig",
    register = "register_external_config"
)]
impl ExternalConfig {
    vela_fields! {
        item: &ExternalItem = ExternalConfig::item(self);
    }
}

#[derive(Debug)]
struct ExternalItem {
    count: i32,
    quality: ExternalQuality,
}

#[external_host(path = "generated::ExternalItem", register = "register_external_item")]
impl ExternalItem {
    vela_fields! {
        count: i32 = self.count;
        quality: ExternalQuality = self.quality;
    }
}

#[methods(path = "game::FieldChild")]
impl FieldChild {}

#[methods(path = "game::Team")]
impl Team {
    pub fn total(&self) -> i64 {
        self.first.level + self.second.level
    }

    pub fn players_mut(&mut self) -> (&mut Player, &mut Player) {
        (&mut self.first, &mut self.second)
    }
}

#[methods(path = "game::PlayerService")]
impl PlayerService {
    pub fn touch_count(&self) -> i64 {
        self.touches
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    pub fn player_mut(&mut self) -> &mut Player {
        &mut self.player
    }

    pub fn maybe_player(&self, present: bool) -> Option<&Player> {
        present.then_some(&self.player)
    }

    pub fn checked_player(&self, allowed: bool) -> Result<&Player, i64> {
        if allowed { Ok(&self.player) } else { Err(42) }
    }
}

#[trait_export(path = "game::Damageable")]
pub trait Damageable {
    fn take_damage(&mut self, amount: i64);
    fn is_alive(&self) -> bool;
}

#[methods(path = "game::Player")]
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

    #[vela(effects(random))]
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

#[export(path = "game::service_player")]
pub fn service_player(service: &PlayerService) -> &Player {
    &service.player
}

#[export(path = "game::service_player_mut")]
pub fn service_player_mut(service: &mut PlayerService) -> &mut Player {
    &mut service.player
}

#[export(path = "game::maybe_service_player")]
pub fn maybe_service_player(service: &PlayerService, present: bool) -> Option<&Player> {
    present.then_some(&service.player)
}

#[export(path = "game::checked_service_player")]
pub fn checked_service_player(service: &PlayerService, allowed: bool) -> Result<&Player, i64> {
    if allowed {
        Ok(&service.player)
    } else {
        Err(41)
    }
}

#[export(path = "game::fallible_service_player")]
pub fn fallible_service_player(service: &PlayerService, allowed: bool) -> VmResult<&Player> {
    allowed.then_some(&service.player).ok_or_else(|| {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: "fallible borrowed host return",
        })
    })
}

#[export(path = "game::split_team")]
pub fn split_team(team: &mut Team) -> (&mut Player, &mut Player) {
    (&mut team.first, &mut team.second)
}

#[export(path = "game::split_team_if")]
pub fn split_team_if(team: &mut Team, enabled: bool) -> Option<(&mut Player, &mut Player)> {
    enabled.then_some((&mut team.first, &mut team.second))
}

#[export(path = "game::team_total")]
pub fn team_total(team: &Team) -> i64 {
    team.first.level + team.second.level
}

#[export(path = "game::touch_service")]
pub fn touch_service(service: &mut PlayerService) -> i64 {
    service.touches += 1;
    service.touches
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

#[export(path = "game::double_async")]
pub async fn double_async(amount: i64) -> i64 {
    amount * 2
}

#[export(path = "game::transfer_async")]
pub async fn transfer_async(first: &mut Player, second: &mut Player, amount: i64) -> i64 {
    first.level -= amount;
    second.level += amount;
    first.level + second.level
}

#[export(path = "game::hold_player_async")]
pub async fn hold_player_async(_player: &mut Player) {
    std::future::pending::<()>().await;
}

#[export(path = "game::roll", effects(random))]
pub fn roll(_ctx: &mut NativeCallContext<'_, '_>, player: &Player) -> i64 {
    player.level
}

#[methods(path = "game::Player")]
impl Player {
    #[vela(attr = "context_operations=inspect")]
    pub fn current_level(&self) -> i64 {
        self.level
    }

    pub fn increment(&mut self, amount: i64) {
        self.level += amount;
    }

    pub fn absorb(&mut self, other: &mut Player) -> i64 {
        self.level += other.level;
        other.level = 0;
        self.level
    }

    pub fn combined(&self, other: &Player) -> i64 {
        self.level + other.level
    }

    pub async fn increment_async(&mut self, amount: i64) -> i64 {
        self.level += amount;
        self.level
    }

    pub async fn absorb_async(&mut self, other: &mut Player) -> i64 {
        self.level += other.level;
        other.level = 0;
        self.level
    }

    pub async fn hold_async(&mut self) {
        std::future::pending::<()>().await;
    }

    pub async fn context_increment_async(
        &mut self,
        context: &mut NativeCallContext<'_, '_>,
        amount: i64,
    ) -> VmResult<i64> {
        context.charge_execution_units(1)?;
        self.level += amount;
        Ok(self.level)
    }

    fn rust_only_helper(&self) -> i64 {
        self.level
    }
}

pub trait ExternalDamage {
    fn hit(&mut self, amount: i64);
    fn active(&self) -> bool;
}

#[derive(Debug, ScriptHost)]
#[vela(path = "external::Npc")]
pub struct ExternalNpc {
    #[vela(get, set)]
    hp: i64,
}

impl ExternalDamage for ExternalNpc {
    fn hit(&mut self, amount: i64) {
        self.hp -= amount.max(0);
    }

    fn active(&self) -> bool {
        self.hp > 0
    }
}

#[methods(path = "external::Npc")]
impl ExternalNpc {
    pub fn current_hp(&self) -> i64 {
        self.hp
    }
}

export_external_trait_impl! {
    type ExternalNpc;
    trait ExternalDamage as "external::Damage";
    fn hit(&mut self, amount: i64);
    fn active(&self) -> bool;
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
    assert_eq!(
        shared.attrs.get("context_operations").map(String::as_str),
        Some("inspect")
    );
    assert_eq!(
        shared
            .native_method_desc(Player::vela_host_type_desc().key)
            .attrs
            .get("context_operations"),
        Some("inspect")
    );
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
    let bundle = Player::vela_protocol_Damageable_methods();

    assert_eq!(protocol.identity.public_path, "game::Damageable");
    assert_eq!(protocol.methods.len(), 2);
    assert_eq!(protocol.methods[0].effects, EffectSet::host_write());
    assert_eq!(protocol.methods[1].effects, EffectSet::host_read());
    assert_eq!(bundle.protocols(), std::slice::from_ref(&protocol));

    let mut player = Player { level: 5 };
    player.take_damage(2);
    assert!(player.is_alive());
}

#[test]
fn export_module_groups_public_contracts_once() {
    let bundle = rules_exports::vela_module();
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
fn module_registration_installs_through_application_bindings() {
    let mut bindings = VelaBindings::new();
    bindings.register_module(rules_exports::vela_module());
    let engine = Engine::builder()
        .register_bindings(bindings)
        .build()
        .expect("module registration should install");
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
        .install_generated_type::<Player>()
        .install_generated_type::<PlayerService>()
        .install_generated_type::<Team>()
        .install_registration(vela_function_grant_exp())
        .install_registration(vela_function_sum_levels())
        .install_registration(vela_function_transfer())
        .install_registration(vela_function_mixed_alias())
        .install_registration(vela_function_service_player())
        .install_registration(vela_function_service_player_mut())
        .install_registration(vela_function_maybe_service_player())
        .install_registration(vela_function_checked_service_player())
        .install_registration(vela_function_fallible_service_player())
        .install_registration(vela_function_split_team())
        .install_registration(vela_function_split_team_if())
        .install_registration(vela_function_team_total())
        .install_registration(vela_function_touch_service())
        .install_registration(vela_function_roll())
        .install_registration(vela_function_strict_grant())
        .install_registration(vela_function_fail_grant())
        .install_registration(vela_function_panic_grant())
        .install_registration(vela_function_double_async())
        .install_registration(vela_function_transfer_async())
        .install_registration(vela_function_hold_player_async())
        .install_registration(Player::vela_methods())
        .install_registration(PlayerService::vela_methods())
        .install_registration(Team::vela_methods())
        .install_registration(Player::vela_protocol_Damageable_methods())
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
fn generated_sync_method_uses_controlled_adapter_for_fallback_host_refs() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .install_generated_type::<Player>()
        .install_registration(Player::vela_methods())
        .build()
        .expect("generated Player methods should register");
    let program = engine
        .compile_source("fn main(player: Player) { player.increment(3); return 7; }")
        .expect("fallback HostRef method call should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let method = vela_common::HostMethodId::new(u128::from(vela_common::stable_id(
        "host_method",
        Player::vela_stable_type_path(),
        "increment",
    )));
    let root = HostRef::new(
        Player::vela_host_type_id(),
        vela_common::HostObjectId::new(41),
        1,
    );
    let mut adapter = MockStateAdapter::new();
    adapter.insert_method_return(method, HostValue::Unit);
    let mut args = CallArgs::new();
    args.push_host_handle("player", root);

    let result = runtime
        .call(
            "main",
            args.with_fallback_adapter(&mut adapter),
            CallOptions::unbounded(),
        )
        .expect("generated method should use the controlled fallback adapter");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(7)));
    assert_eq!(
        adapter
            .method_calls()
            .iter()
            .filter(|call| call.method == method)
            .count(),
        1
    );
}

#[test]
fn empty_inherent_method_group_supports_field_only_host_objects() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .install_generated_type::<FieldOnly>()
        .install_registration(FieldOnly::vela_methods())
        .install_generated_type::<FieldChild>()
        .install_registration(FieldChild::vela_methods())
        .build()
        .expect("field-only host type should register");
    let program = engine
        .compile_source("fn main(value: FieldOnly) { return value.child.value; }")
        .expect("field getter should compile");
    let mut runtime = Runtime::new(engine, program).expect("field-only runtime should initialize");
    let value = FieldOnly {
        child: FieldChild { value: 42 },
    };
    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("value", &value),
            CallOptions::unbounded(),
        )
        .expect("field-only host object should cross the root boundary");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(42)));
}

#[test]
fn centralized_external_host_binding_supports_borrowed_children() {
    let mut bindings = VelaBindings::new();
    bindings.register_type(TypeRegistration::<ExternalQuality>::of());
    register_external_config(&mut bindings);
    register_external_item(&mut bindings);
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_bindings(bindings)
        .build()
        .expect("centralized external bindings should register");
    let program = engine
        .compile_source("fn main(config: ExternalConfig) { return config.item.count; }")
        .expect("external Host properties should compile");
    let mut runtime =
        Runtime::new(engine, program).expect("external Host runtime should initialize");
    let config = ExternalConfig {
        item: ExternalItem {
            count: 42,
            quality: ExternalQuality::Rare,
        },
    };
    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("config", &config),
            CallOptions::unbounded(),
        )
        .expect("borrowed external child should remain call-scoped");

    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I32(42)))
    );
}

#[test]
fn centralized_external_host_properties_preserve_owned_enum_values() {
    let mut bindings = VelaBindings::new();
    bindings.register_type(TypeRegistration::<ExternalQuality>::of());
    register_external_config(&mut bindings);
    register_external_item(&mut bindings);
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .register_bindings(bindings)
        .build()
        .expect("centralized external properties should register");
    let program = engine
        .compile_source("fn main(item: ExternalItem) { return item.quality; }")
        .expect("external enum property should compile");
    let mut runtime =
        Runtime::new(engine, program).expect("external property runtime should initialize");
    let item = ExternalItem {
        count: 42,
        quality: ExternalQuality::Rare,
    };
    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("item", &item),
            CallOptions::unbounded(),
        )
        .expect("external enum property should retain its Value representation");

    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::enum_variant(
            "generated::ExternalQuality",
            "Rare",
            std::iter::empty::<(&str, OwnedValue)>(),
        ))
    );
}

#[test]
fn shared_borrowed_return_freezes_owner_and_behaves_as_host_ref() {
    let mut runtime = host_export_runtime(
        "fn main(service: PlayerService) { let first = game::service_player(service); let second = game::service_player(service); return first.current_level() + second.current_level(); }",
    );
    let service = PlayerService {
        player: Player { level: 6 },
        touches: 0,
    };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("service", &service),
            CallOptions::unbounded(),
        )
        .expect("shared borrowed children should support ordinary host methods");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
}

#[test]
fn shared_borrowed_return_rejects_owner_write_and_cleans_up_at_root_end() {
    let mut runtime = host_export_runtime(
        "fn blocked(service: PlayerService) { let player = game::service_player(service); game::touch_service(service); return player.current_level(); } fn after(service: PlayerService) { return game::touch_service(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 6 },
        touches: 0,
    };

    let error = runtime
        .call(
            "blocked",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("a live shared-origin child must freeze owner writes");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::HostObjectBusy { .. })
    ));

    let result = runtime
        .call(
            "after",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("root cleanup must release the retained owner lease");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(1)));
    assert_eq!(service.touches, 1);
}

#[test]
fn exclusive_borrowed_return_requires_explicit_release_after_last_use() {
    let mut runtime = host_export_runtime(
        "fn blocked(service: PlayerService) { let player = game::service_player_mut(service); player.increment(2); return game::touch_service(service); } fn explicit(service: PlayerService) { let player = game::service_player_mut(service); player.increment(2); host::release(player); return game::touch_service(service); } fn via_method(service: PlayerService) { let player = service.player_mut(); player.increment(3); return player.current_level(); }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };

    let error = runtime
        .call(
            "blocked",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("proven last use must not release a scoped child");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::HostObjectBusy { .. })
    ));
    assert_eq!(service.player.level, 7);
    assert_eq!(service.touches, 0);

    let result = runtime
        .call(
            "explicit",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("authored release should unfreeze the owner");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(1)));
    assert_eq!(service.player.level, 9);
    assert_eq!(service.touches, 1);

    let result = runtime
        .call(
            "via_method",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("borrowed-return methods should use the same scoped adapter");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(12)));
    assert_eq!(service.player.level, 12);
}

#[test]
fn aliased_borrowed_return_remains_frozen_without_explicit_release() {
    let mut runtime = host_export_runtime(
        "fn main(service: PlayerService) { let player = service.player_mut(); let alias = player; player.increment(2); return game::touch_service(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };

    let error = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("an unreleased alias group must keep the owner frozen");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::HostObjectBusy { .. })
    ));
    assert_eq!(service.player.level, 7);
}

#[test]
fn borrowed_return_remains_frozen_after_scope_and_branch_end() {
    let mut runtime = host_export_runtime(
        "fn lexical(service: PlayerService) { { let player = service.player_mut(); player.increment(1); } return game::touch_service(service); } fn branch(service: PlayerService, flag: bool) { let player = service.player_mut(); if flag { player.increment(2); } else { player.increment(3); } return game::touch_service(service); } fn one_branch(service: PlayerService, flag: bool) { let player = service.player_mut(); if flag { player.increment(4); } return game::touch_service(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };

    for (function, flag) in [
        ("lexical", None),
        ("branch", Some(true)),
        ("branch", Some(false)),
        ("one_branch", Some(true)),
        ("one_branch", Some(false)),
    ] {
        let mut args = CallArgs::new().with_host_mut("service", &mut service);
        if let Some(flag) = flag {
            args = args.with_value("flag", flag);
        }
        let error = runtime
            .call(function, args, CallOptions::unbounded())
            .expect_err("scope and branch convergence must not release scoped borrows");
        assert!(matches!(
            error.kind(),
            VmErrorKind::Host(vela_host::error::HostErrorKind::HostObjectBusy { .. })
        ));
    }

    assert_eq!(service.player.level, 15);
    assert_eq!(service.touches, 0);
}

#[test]
fn host_release_invalidates_alias_group_and_unfreezes_owner() {
    let mut runtime = host_export_runtime(
        "fn release_then_touch(service: PlayerService) { let player = service.player_mut(); let alias = player; host::release(player); return game::touch_service(service); } fn use_expired(service: PlayerService) { let player = service.player(); let alias = player; host::release(player); return alias.current_level(); } fn release_root(service: PlayerService) { host::release(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };

    let result = runtime
        .call(
            "release_then_touch",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("explicit release should immediately unfreeze the owner");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(1)));

    let error = runtime
        .call(
            "use_expired",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("all aliases of a released child must expire");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::ExpiredBorrowedHostRef { .. })
    ));

    let error = runtime
        .call(
            "release_root",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("ordinary root HostRefs are not scoped borrows");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::NotScopedBorrow { .. })
    ));
}

#[test]
fn host_try_release_is_idempotent_but_preserves_strict_errors() {
    let mut runtime = host_export_runtime(
        "fn live(service: PlayerService) { let player = service.player_mut(); let released = host::try_release(player); game::touch_service(service); return released; } \
         fn expired(service: PlayerService) { let player = service.player_mut(); let alias = player; host::release(player); return host::try_release(alias); } \
         fn strict_after_try(service: PlayerService) { let player = service.player_mut(); let alias = player; host::try_release(player); host::release(alias); } \
         fn root(service: PlayerService) { return host::try_release(service); } \
         fn converge(service: PlayerService, early: bool) { let player = service.player_mut(); if early { host::release(player); } let released = host::try_release(player); game::touch_service(service); return released; }",
    );
    let mut service = PlayerService {
        player: Player { level: 5 },
        touches: 0,
    };

    let result = runtime
        .call(
            "live",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("try-release should close a live scoped group");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(true)));
    assert_eq!(service.touches, 1);

    let result = runtime
        .call(
            "expired",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("try-release should suppress only known expiry");
    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::Bool(false)));

    let error = runtime
        .call(
            "strict_after_try",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("strict release after try-release must still report expiry");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::ExpiredBorrowedHostRef { .. })
    ));

    let error = runtime
        .call(
            "root",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("try-release must reject ordinary root hosts");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::NotScopedBorrow { .. })
    ));

    for (early, expected) in [(true, false), (false, true)] {
        let result = runtime
            .call(
                "converge",
                CallArgs::new()
                    .with_host_mut("service", &mut service)
                    .with_value("early", early),
                CallOptions::unbounded(),
            )
            .expect("one try-release should converge path-dependent release state");
        assert_eq!(
            runtime.value_to_owned(&result),
            Ok(OwnedValue::Bool(expected))
        );
    }
    assert_eq!(service.touches, 3);
}

#[test]
fn discarded_scoped_producer_results_fail_before_execution() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .install_generated_type::<Player>()
        .install_generated_type::<PlayerService>()
        .install_registration(Player::vela_methods())
        .install_registration(PlayerService::vela_methods())
        .install_registration(vela_function_service_player_mut())
        .build()
        .expect("scoped producer fixture should register");

    for source in [
        "fn main(service: PlayerService) { service.player_mut(); }",
        "fn main(service: PlayerService) { game::service_player_mut(service); }",
    ] {
        let error = engine
            .compile_source(source)
            .expect_err("discarded scoped results must fail during compilation");
        assert!(matches!(
            error.kind,
            EngineSourceErrorKind::Backend(ref error)
                if matches!(
                    error.kind,
                    CompileErrorKind::DiscardedScopedResource { .. }
                ) && error
                    .to_diagnostic()
                    .is_some_and(|diagnostic| diagnostic.code.as_deref()
                        == Some("compiler::discarded_scoped_resource"))
        ));
    }
}

#[test]
fn unnamed_scoped_receiver_chains_fail_before_execution() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .install_generated_type::<Player>()
        .install_generated_type::<PlayerService>()
        .install_registration(Player::vela_methods())
        .install_registration(PlayerService::vela_methods())
        .build()
        .expect("scoped producer fixture should register");

    let error = engine
        .compile_source(
            "fn main(service: PlayerService) { return service.player_mut().current_level(); }",
        )
        .expect_err("unnamed scoped receivers must fail during compilation");
    let diagnostic = match &error.kind {
        EngineSourceErrorKind::Backend(error) => error
            .to_diagnostic()
            .expect("scoped resource error should be diagnostic"),
        other => panic!("expected backend diagnostic, found {other:?}"),
    };
    assert!(diagnostic.message.contains("MutView"));
    assert!(diagnostic.message.contains("receiver"));
    assert!(matches!(
        error.kind,
        EngineSourceErrorKind::Backend(ref error)
            if matches!(
                error.kind,
                CompileErrorKind::UnnameableScopedResource { .. }
            ) && error
                .to_diagnostic()
                .is_some_and(|diagnostic| diagnostic.code.as_deref()
                    == Some("compiler::unnameable_scoped_resource"))
    ));
}

#[test]
fn authored_release_intrinsics_compile_to_dedicated_instructions() {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .install_generated_type::<Player>()
        .install_generated_type::<PlayerService>()
        .install_registration(Player::vela_methods())
        .install_registration(PlayerService::vela_methods())
        .build()
        .expect("release instruction fixture should register");
    let program = engine
        .compile_source(
            "fn strict(service: PlayerService) { let player = service.player_mut(); host::release(player); } fn idempotent(service: PlayerService) { let player = service.player_mut(); return host::try_release(player); } fn implicit(service: PlayerService) { let player = service.player_mut(); player.increment(1); }",
        )
        .expect("release instruction fixture should compile");

    let strict = program
        .bytecode()
        .function("strict")
        .expect("strict function should exist");
    assert!(strict.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::ReleaseBorrowLease { .. }
    )));
    assert!(!strict.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallNative { native, .. }
            if native == vela_def::host_release_function_id()
    )));

    let idempotent = program
        .bytecode()
        .function("idempotent")
        .expect("idempotent function should exist");
    assert!(idempotent.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::TryReleaseBorrowLease { .. }
    )));
    assert!(!idempotent.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallNative { native, .. }
            if native == vela_def::host_try_release_function_id()
    )));

    let implicit = program
        .bytecode()
        .function("implicit")
        .expect("implicit function should exist");
    assert!(!implicit.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::ReleaseBorrowLease { .. }
            | UnlinkedInstructionKind::TryReleaseBorrowLease { .. }
    )));
}

#[test]
fn option_and_result_borrowed_returns_retain_only_success_children() {
    let mut runtime = host_export_runtime(
        "fn some(service: PlayerService) { let player = game::maybe_service_player(service, true)?; return Option::Some(player.current_level()); } fn none(service: PlayerService) { let player = game::maybe_service_player(service, false)?; return Option::Some(player.current_level()); } fn ok(service: PlayerService) { let player = game::checked_service_player(service, true)?; return Result::Ok(player.current_level()); } fn err(service: PlayerService) { let player = game::checked_service_player(service, false)?; return Result::Ok(player.current_level()); }",
    );
    let service = PlayerService {
        player: Player { level: 8 },
        touches: 0,
    };

    for (entry, variant) in [
        ("some", "Some"),
        ("none", "None"),
        ("ok", "Ok"),
        ("err", "Err"),
    ] {
        let value = runtime
            .call(
                entry,
                CallArgs::new().with_host_ref("service", &service),
                CallOptions::unbounded(),
            )
            .expect("structured borrowed return branch should execute");
        let owned = runtime
            .value_to_owned(&value)
            .expect("result should materialize");
        assert!(matches!(
            owned,
            OwnedValue::Enum { variant: ref actual, .. } if actual == variant
        ));
    }
}

#[test]
fn option_and_result_borrowed_method_returns_match_free_functions() {
    let mut runtime = host_export_runtime(
        "fn some(service: PlayerService) { let player = service.maybe_player(true)?; return Option::Some(player.current_level()); } fn none(service: PlayerService) { let player = service.maybe_player(false)?; return Option::Some(player.current_level()); } fn ok(service: PlayerService) { let player = service.checked_player(true)?; return Result::Ok(player.current_level()); } fn err(service: PlayerService) { let player = service.checked_player(false)?; return Result::Ok(player.current_level()); }",
    );
    let service = PlayerService {
        player: Player { level: 9 },
        touches: 0,
    };

    for (entry, variant) in [
        ("some", "Some"),
        ("none", "None"),
        ("ok", "Ok"),
        ("err", "Err"),
    ] {
        let value = runtime
            .call(
                entry,
                CallArgs::new().with_host_ref("service", &service),
                CallOptions::unbounded(),
            )
            .expect("structured borrowed method return branch should execute");
        let owned = runtime
            .value_to_owned(&value)
            .expect("result should materialize");
        assert!(matches!(
            owned,
            OwnedValue::Enum { variant: ref actual, .. } if actual == variant
        ));
    }
}

#[test]
fn vm_result_borrowed_return_releases_owner_on_error() {
    let mut runtime = host_export_runtime(
        "fn ok(service: PlayerService) { let player = game::fallible_service_player(service, true); return player.current_level(); } fn fail(service: PlayerService) { let player = game::fallible_service_player(service, false); return player.current_level(); } fn after(service: PlayerService) { return game::touch_service(service); }",
    );
    let mut service = PlayerService {
        player: Player { level: 8 },
        touches: 0,
    };
    let value = runtime
        .call(
            "ok",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("VmResult success should retain the borrowed child");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(8)));

    let error = runtime
        .call(
            "fail",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect_err("VmResult error should cross the boundary");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "fallible borrowed host return"
        }
    ));
    let value = runtime
        .call(
            "after",
            CallArgs::new().with_host_mut("service", &mut service),
            CallOptions::unbounded(),
        )
        .expect("error cleanup must release the owner lease");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(1)));
}

#[test]
fn tuple_borrowed_return_creates_distinct_siblings_under_one_freeze() {
    let mut runtime = host_export_runtime(
        "fn main(team: Team) { let pair = game::split_team(team); let first = pair.0; let second = pair.1; first.increment(2); second.increment(3); let nested = game::sum_levels(first, second); host::release(first); host::release(second); return game::team_total(team) + nested; }",
    );
    let mut team = Team {
        first: Player { level: 4 },
        second: Player { level: 6 },
        marker: 0,
    };

    let value = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("team", &mut team),
            CallOptions::unbounded(),
        )
        .expect("releasing every tuple sibling should unfreeze the owner");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(30)));
    assert_eq!(team.first.level, 6);
    assert_eq!(team.second.level, 9);
}

#[test]
fn option_tuple_borrowed_return_retains_only_the_some_group() {
    let mut runtime = host_export_runtime(
        "fn some(team: Team) { let pair = game::split_team_if(team, true)?; let first = pair.0; let second = pair.1; first.increment(1); host::release(first); host::release(second); return Option::Some(game::team_total(team)); } fn none(team: Team) { return game::split_team_if(team, false); }",
    );
    let mut team = Team {
        first: Player { level: 2 },
        second: Player { level: 3 },
        marker: 0,
    };

    for (entry, variant) in [("some", "Some"), ("none", "None")] {
        let value = runtime
            .call(
                entry,
                CallArgs::new().with_host_mut("team", &mut team),
                CallOptions::unbounded(),
            )
            .expect("optional tuple branch should execute");
        let owned = runtime
            .value_to_owned(&value)
            .expect("value should materialize");
        assert!(matches!(
            owned,
            OwnedValue::Enum { variant: ref actual, .. } if actual == variant
        ));
    }
}

#[test]
fn tuple_borrowed_method_return_uses_the_same_sibling_group_model() {
    let mut runtime = host_export_runtime(
        "fn main(team: Team) { let pair = team.players_mut(); let first = pair.0; let second = pair.1; first.increment(4); second.increment(5); host::release(first); host::release(second); return team.total(); }",
    );
    let mut team = Team {
        first: Player { level: 1 },
        second: Player { level: 2 },
        marker: 0,
    };

    let value = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("team", &mut team),
            CallOptions::unbounded(),
        )
        .expect("tuple method siblings should release independently");
    assert_eq!(runtime.value_to_owned(&value), Ok(OwnedValue::i64(12)));
}

#[test]
fn bare_release_names_are_not_registered() {
    let engine = Engine::builder().build().expect("engine should build");
    for name in ["release", "try_release"] {
        let error = engine
            .compile_source(&format!("fn main(value) {{ {name}(value); }}"))
            .expect_err("only namespaced host release intrinsics are reserved");
        assert!(error.to_string().contains(name));
    }
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

#[test]
fn inherent_method_exports_use_ordinary_vela_method_syntax() {
    let mut runtime = host_export_runtime(
        "fn main(player: Player) { player.increment(4); return player.current_level(); }",
    );
    let mut player = Player { level: 5 };

    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect("registered ordinary methods should execute");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(9)));
    assert_eq!(player.level, 9);
}

#[test]
fn inherent_method_exports_apply_alias_matrix_to_receiver_and_parameters() {
    let mut distinct_runtime = host_export_runtime(
        "fn main(first: Player, second: Player) { return first.absorb(second); }",
    );
    let mut first = Player { level: 5 };
    let mut second = Player { level: 4 };
    let result = distinct_runtime
        .call(
            "main",
            CallArgs::new()
                .with_host_mut("first", &mut first)
                .with_host_mut("second", &mut second),
            CallOptions::unbounded(),
        )
        .expect("distinct mutable receiver and parameter should run");
    assert_eq!(
        distinct_runtime.value_to_owned(&result),
        Ok(OwnedValue::i64(9))
    );
    assert_eq!((first.level, second.level), (9, 0));

    let mut shared_runtime =
        host_export_runtime("fn main(player: Player) { return player.combined(player); }");
    let player = Player { level: 7 };
    let result = shared_runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("player", &player),
            CallOptions::unbounded(),
        )
        .expect("shared receiver and shared parameter alias should run");
    assert_eq!(
        shared_runtime.value_to_owned(&result),
        Ok(OwnedValue::i64(14))
    );

    let mut aliased_runtime =
        host_export_runtime("fn main(player: Player) { return player.absorb(player); }");
    let mut player = Player { level: 7 };
    let error = aliased_runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("player", &mut player),
            CallOptions::unbounded(),
        )
        .expect_err("mutable receiver alias must fail before authored Rust");
    assert_eq!(
        error.kind(),
        VmErrorKind::AliasedMutableHostArguments {
            callable: "game::Player::absorb".to_owned(),
            first_parameter: "self".to_owned(),
            second_parameter: "other".to_owned(),
        }
    );
    assert_eq!(player.level, 7);
}

mod interop_exports_async_and_traits;
