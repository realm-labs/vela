use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use vela_common::SourceId;
use vela_def::FunctionId;
use vela_engine::args::{FromScriptArg, IntoScriptArg};
use vela_engine::engine::Engine;
use vela_engine::native::{EffectSet, NativeFunctionDesc, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::runtime::CallOptions;
use vela_engine::service::{
    LinkedServiceSourceManifest, PatchEdit, PatchSources, Service, ServicePatch,
    ServiceSourceManifest, ServiceUpdateBundle,
};
use vela_engine::type_binding::TypeBinding;
use vela_hir::source_ingestion::build_single_source;
use vela_macros::{ScriptHost, Value, methods, service, service_domain};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

const RULE_SOURCE: &str = r#"
#[service_impl(fixture::grant_rule)]
impl GrantRulePatch {
    fn normalize_count(turn, count, multiplier) {
        let normalized = service::base::normalize_count(turn, count, multiplier)?;
        return Result::Ok(normalized + 1i32);
    }
}
"#;

const REWARD_SOURCE: &str = r#"
#[service_impl(fixture::reward)]
impl RewardPatch {
    fn apply(turn, grouped, labels) {
        let result = service::base::apply(turn, grouped, labels);
        let adjustment = fixture::PatchAdjustment::new(2i32, "reward-delta");
        let classes = grouped.group_by(|key, value|
            if value >= 5i32 { "large" } else { "small" });
        let actor = turn.actor_mut();
        actor.record_patch(adjustment.bonus, classes.len());
        let counts = actor.item_counts_mut();
        counts.insert(-1i32, adjustment.bonus);
        host::release(counts);
        host::release(actor);
        return result;
    }
}
"#;

const INVENTORY_SOURCE: &str = r#"
#[service_impl(fixture::inventory)]
impl InventoryPatch {
    fn grant(turn, items, multipliers) {
        let groups = items.group_by(|item| item.template_id);
        let first = items[0];
        let multiplier = multipliers.get(first.template_id).unwrap_or(1i32);
        let preview = service::pinned::rule::normalize_count(turn, first.count, multiplier)?;
        let granted = service::base::grant(turn, items, multipliers)?;
        turn.record_preview(preview, groups.len());
        service::pinned::events::record(turn, groups.len())?;
        return Result::Ok(granted);
    }
}
"#;

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Value)]
#[vela(path = "fixture::ServiceError")]
pub struct ServiceError {
    message: String,
}

impl ServiceError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ServiceError {}

#[derive(Clone, Debug, Value)]
#[vela(path = "fixture::ItemGrant")]
pub struct ItemGrant {
    template_id: i32,
    count: i32,
    tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Value)]
#[vela(path = "fixture::DisplayItem")]
pub struct DisplayItem {
    template_id: i32,
    count: i32,
    label: String,
}

#[derive(Clone, Debug, Value)]
#[vela(path = "fixture::GrantRequest")]
pub struct GrantRequest {
    items: Vec<ItemGrant>,
    multipliers: BTreeMap<i32, i32>,
}

#[derive(Clone, Debug, Value)]
#[vela(path = "fixture::GrantResponse")]
pub struct GrantResponse {
    granted: Vec<DisplayItem>,
}

#[derive(Debug, Value)]
#[vela(path = "fixture::PatchAdjustment")]
struct PatchAdjustment {
    bonus: i32,
    label: String,
}

fn patch_adjustment_binding() -> TypeBinding<PatchAdjustment> {
    let binding = PatchAdjustment::vela_type_binding();
    let key = binding.type_desc().key.clone();
    binding.constructor_fn(
        NativeFunctionDesc::new(
            "fixture::PatchAdjustment::new",
            FunctionId::new(
                vela_common::stable_id("fixture_constructor", "fixture::PatchAdjustment", "new")
                    .into(),
            ),
        )
        .param("bonus", TypeHint::i32())
        .param("label", TypeHint::string())
        .returns(TypeHint::Record(key))
        .effects(EffectSet::pure()),
        construct_patch_adjustment,
    )
}

fn construct_patch_adjustment(
    args: &[OwnedValue],
    _host: &mut vela_vm::HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let [bonus, label] = args else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "fixture::PatchAdjustment::new arguments",
        }));
    };
    Ok(PatchAdjustment {
        bonus: i32::from_script_arg(bonus)?,
        label: String::from_script_arg(label)?,
    }
    .into_script_arg())
}

#[derive(Debug, ScriptHost)]
#[vela(path = "fixture::HostActor")]
pub struct HostActor {
    #[vela(skip)]
    item_counts: BTreeMap<i32, i32>,
    #[vela(skip)]
    last_reward_count: usize,
    #[vela(skip)]
    event_calls: usize,
    #[vela(skip)]
    patch_score: i64,
}

#[methods(path = "fixture::HostActor")]
impl HostActor {
    pub fn item_counts_mut(&mut self) -> &mut BTreeMap<i32, i32> {
        &mut self.item_counts
    }

    pub fn record_patch(&mut self, bonus: i32, group_count: i64) {
        self.patch_score += i64::from(bonus) + group_count;
    }
}

#[derive(ScriptHost)]
#[vela(path = "fixture::HostTurn")]
pub struct HostTurn {
    #[vela(skip)]
    actor: HostActor,
    #[vela(skip)]
    services: GameServicesRoot,
    #[vela(skip)]
    preview_count: i32,
    #[vela(skip)]
    observed_groups: usize,
}

#[methods(path = "fixture::HostTurn")]
impl HostTurn {
    pub fn actor_mut(&mut self) -> &mut HostActor {
        &mut self.actor
    }

    pub fn record_preview(&mut self, preview: i32, groups: i64) {
        self.preview_count = preview;
        self.observed_groups = usize::try_from(groups).unwrap_or_default();
    }
}

#[service(path = "fixture::reward")]
pub trait RewardService: Send + Sync {
    fn apply(
        &self,
        turn: &mut HostTurn,
        grouped: &BTreeMap<i32, i32>,
        labels: &BTreeMap<i32, String>,
    ) -> ServiceResult<Vec<DisplayItem>>;
}

#[service(path = "fixture::inventory")]
pub trait InventoryService: Send + Sync {
    fn grant(
        &self,
        turn: &mut HostTurn,
        items: &[ItemGrant],
        multipliers: &BTreeMap<i32, i32>,
    ) -> ServiceResult<Vec<DisplayItem>>;

    fn current_count(&self, turn: &HostTurn, template_id: i32) -> i32;
}

#[service(path = "fixture::grant_rule")]
pub trait GrantRuleService: Send + Sync {
    fn normalize_count(
        &self,
        turn: &mut HostTurn,
        count: i32,
        multiplier: i32,
    ) -> ServiceResult<i32>;
}

#[service(path = "fixture::grant_event")]
pub trait GrantEventService: Send + Sync {
    fn record(&self, turn: &mut HostTurn, granted_count: i64) -> ServiceResult<()>;
}

#[service(path = "fixture::grant_handler")]
pub trait GrantHandlerService: Send + Sync {
    async fn handle(
        &self,
        turn: &mut HostTurn,
        request: GrantRequest,
    ) -> ServiceResult<GrantResponse>;
}

struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(
        &self,
        turn: &mut HostTurn,
        grouped: &BTreeMap<i32, i32>,
        labels: &BTreeMap<i32, String>,
    ) -> ServiceResult<Vec<DisplayItem>> {
        let mut granted = Vec::with_capacity(grouped.len());
        for (&template_id, &count) in grouped {
            if count <= 0 {
                return Err(ServiceError::new("reward count must be positive"));
            }
            *turn.actor.item_counts.entry(template_id).or_default() += count;
            granted.push(DisplayItem {
                template_id,
                count,
                label: labels
                    .get(&template_id)
                    .cloned()
                    .unwrap_or_else(|| format!("item-{template_id}")),
            });
        }
        Ok(granted)
    }
}

struct RustInventoryService;

impl InventoryService for RustInventoryService {
    fn grant(
        &self,
        turn: &mut HostTurn,
        items: &[ItemGrant],
        multipliers: &BTreeMap<i32, i32>,
    ) -> ServiceResult<Vec<DisplayItem>> {
        let mut grouped = BTreeMap::<i32, i32>::new();
        let mut labels = BTreeMap::<i32, String>::new();
        let services = turn.services.clone();
        for item in items {
            let multiplier = multipliers.get(&item.template_id).copied().unwrap_or(1);
            let count = services
                .rule()
                .normalize_count(turn, item.count, multiplier)?;
            *grouped.entry(item.template_id).or_default() += count;
            if let Some(label) = item.tags.get("label") {
                labels
                    .entry(item.template_id)
                    .or_insert_with(|| label.clone());
            }
        }

        services.reward().apply(turn, &grouped, &labels)
    }

    fn current_count(&self, turn: &HostTurn, template_id: i32) -> i32 {
        turn.actor
            .item_counts
            .get(&template_id)
            .copied()
            .unwrap_or_default()
    }
}

struct RustGrantRuleService;

impl GrantRuleService for RustGrantRuleService {
    fn normalize_count(
        &self,
        _turn: &mut HostTurn,
        count: i32,
        multiplier: i32,
    ) -> ServiceResult<i32> {
        let normalized = count.saturating_mul(multiplier);
        if normalized <= 0 {
            return Err(ServiceError::new("grant count must be positive"));
        }
        Ok(normalized)
    }
}

struct RustGrantEventService;

impl GrantEventService for RustGrantEventService {
    fn record(&self, turn: &mut HostTurn, granted_count: i64) -> ServiceResult<()> {
        turn.actor.last_reward_count = usize::try_from(granted_count)
            .map_err(|_| ServiceError::new("granted count does not fit usize"))?;
        turn.actor.event_calls += 1;
        Ok(())
    }
}

struct RustGrantHandlerService;

impl GrantHandlerService for RustGrantHandlerService {
    async fn handle(
        &self,
        turn: &mut HostTurn,
        request: GrantRequest,
    ) -> ServiceResult<GrantResponse> {
        std::future::ready(()).await;
        let services = turn.services.clone();
        let granted = services
            .inventory()
            .grant(turn, &request.items, &request.multipliers)?;
        services.events().record(
            turn,
            i64::try_from(granted.len())
                .map_err(|_| ServiceError::new("granted count does not fit i64"))?,
        )?;
        Ok(GrantResponse { granted })
    }
}

#[service_domain(context = HostTurn)]
pub struct GameServices {
    pub inventory: Service<dyn InventoryService>,
    pub reward: Service<dyn RewardService>,
    pub rule: Service<dyn GrantRuleService>,
    pub events: Service<dyn GrantEventService>,
    pub handler: Service<dyn GrantHandlerService>,
}

#[derive(Debug, Eq, PartialEq)]
struct RequestSummary {
    checksum: i64,
    item7: i32,
    marker: i32,
    last_reward_count: usize,
    event_calls: usize,
    patch_score: i64,
    preview_count: i32,
    observed_groups: usize,
}

fn request() -> GrantRequest {
    GrantRequest {
        items: vec![
            ItemGrant {
                template_id: 7,
                count: 2,
                tags: BTreeMap::from([("label".to_owned(), "token".to_owned())]),
            },
            ItemGrant {
                template_id: 7,
                count: 1,
                tags: BTreeMap::new(),
            },
            ItemGrant {
                template_id: 9,
                count: 4,
                tags: BTreeMap::new(),
            },
        ],
        multipliers: BTreeMap::from([(7, 2)]),
    }
}

fn run_pinned(root: &GameServicesRoot) -> Result<(RequestSummary, HostTurn), ServiceError> {
    let mut turn = HostTurn {
        actor: HostActor {
            item_counts: BTreeMap::new(),
            last_reward_count: 0,
            event_calls: 0,
            patch_score: 0,
        },
        services: root.clone(),
        preview_count: 0,
        observed_groups: 0,
    };
    let response = block_on(root.handler().handle(&mut turn, request()))?;
    let checksum = response.granted.iter().fold(0_i64, |checksum, item| {
        checksum
            + i64::from(item.template_id) * 100
            + i64::from(item.count) * 10
            + i64::try_from(item.label.len()).unwrap_or_default()
    });
    let summary = RequestSummary {
        checksum,
        item7: root.inventory().current_count(&turn, 7),
        marker: root.inventory().current_count(&turn, -1),
        last_reward_count: turn.actor.last_reward_count,
        event_calls: turn.actor.event_calls,
        patch_score: turn.actor.patch_score,
        preview_count: turn.preview_count,
        observed_groups: turn.observed_groups,
    };
    Ok((summary, turn))
}

fn run_active(services: &GameServices) -> Result<(RequestSummary, HostTurn), ServiceError> {
    let root = services.pin();
    run_pinned(&root)
}

fn linked_update(
    engine: &Engine,
    services: &GameServices,
    source_id: u32,
    manifest_source: &str,
    artifact_source: &str,
) -> Result<
    (
        Arc<vela_bytecode::LinkedArtifact>,
        LinkedServiceSourceManifest,
    ),
    Box<dyn Error>,
> {
    let sources = build_single_source(SourceId::new(source_id), manifest_source)
        .map_err(|error| format!("{error:?}"))?;
    let manifest = ServiceSourceManifest::link(sources.graph(), services.schema())?;
    let artifact = engine.link_compiled_program(engine.compile_source(artifact_source)?)?;
    let update = manifest.bind_artifact(Arc::clone(&artifact))?;
    Ok((artifact, update))
}

fn call_options() -> CallOptions {
    CallOptions::new(250_000, 4 * 1024 * 1024, 128)
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let app = GameServices::builder(
        Engine::builder()
            .capability(Capability::HostWrite)
            .register_type::<HostActor>()
            .register_type::<HostTurn>()
            .register_exports(HostActor::vela_inherent_exports())
            .register_exports(HostTurn::vela_inherent_exports())
            .register_type_binding::<PatchAdjustment>(patch_adjustment_binding()),
    )
    .inventory(RustInventoryService)
    .reward(RustRewardService)
    .rule(RustGrantRuleService)
    .events(RustGrantEventService)
    .handler(RustGrantHandlerService)
    .call_options(call_options())
    .build()?;
    let engine = app.engine().clone();
    let services = app.domain();
    assert_eq!(
        engine
            .type_bindings()
            .get_for::<PatchAdjustment>()
            .expect("PatchAdjustment binding")
            .constructor_ids
            .len(),
        1
    );

    let rust_root = services.pin();
    let (rust, _) = run_active(services)?;
    assert_eq!(
        rust,
        RequestSummary {
            checksum: 1711,
            item7: 6,
            marker: 0,
            last_reward_count: 2,
            event_calls: 1,
            patch_score: 0,
            preview_count: 0,
            observed_groups: 0,
        }
    );

    app.patches()
        .apply(PatchEdit::put("rule.vela", RULE_SOURCE))?;
    let rule_root = services.pin();
    let (rule, _) = run_active(services)?;
    assert_eq!(rule.checksum, 1741);
    assert_eq!(rule.item7, 8);

    let rule_reward_source = format!("{RULE_SOURCE}\n{REWARD_SOURCE}");
    let (reward_artifact, reward_update) =
        linked_update(&engine, services, 102, REWARD_SOURCE, &rule_reward_source)?;
    let reward_bundle = ServiceUpdateBundle::delta(
        services.schema(),
        rule_root.generation_id(),
        rule_root
            .artifact_checksum()
            .expect("Vela rule generation has an artifact"),
        reward_artifact,
        reward_update,
    )?;
    assert!(app.patches().dry_run_bundle(&reward_bundle).accepted());
    app.patches().stage_bundle(reward_bundle)?.activate()?;
    let reward_root = services.pin();
    let (reward, _) = run_active(services)?;
    assert_eq!(reward.checksum, 1741);
    assert_eq!(reward.marker, 2);
    assert_eq!(reward.patch_score, 3);

    let complete_source = format!("{RULE_SOURCE}\n{REWARD_SOURCE}\n{INVENTORY_SOURCE}");
    let (inventory_artifact, inventory_update) =
        linked_update(&engine, services, 103, INVENTORY_SOURCE, &complete_source)?;
    let inventory_bundle = ServiceUpdateBundle::delta(
        services.schema(),
        reward_root.generation_id(),
        reward_root
            .artifact_checksum()
            .expect("Vela reward generation has an artifact"),
        inventory_artifact,
        inventory_update,
    )?;
    assert!(app.patches().dry_run_bundle(&inventory_bundle).accepted());
    app.patches().stage_bundle(inventory_bundle)?.activate()?;
    let complete_root = services.pin();
    let (complete, _) = run_active(services)?;
    assert_eq!(
        complete,
        RequestSummary {
            checksum: 1741,
            item7: 8,
            marker: 2,
            last_reward_count: 2,
            event_calls: 2,
            patch_score: 3,
            preview_count: 5,
            observed_groups: 2,
        }
    );
    assert_eq!(
        complete_root
            .selections()
            .expect("complete Vela generation")
            .iter()
            .filter(|(_, selection)| matches!(
                selection,
                vela_engine::service::ServiceMethodSelection::Vela(_)
            ))
            .count(),
        3
    );

    let (old_rust, _) = run_pinned(&rust_root)?;
    assert_eq!(old_rust.checksum, 1711);
    let (old_rule, _) = run_pinned(&rule_root)?;
    assert_eq!(old_rule.checksum, 1741);
    assert_eq!(old_rule.marker, 0);

    let rollback = app
        .patches()
        .stage(ServicePatch::replace(PatchSources::from_files([
            ("rule.vela", RULE_SOURCE),
            ("reward.vela", REWARD_SOURCE),
            ("inventory.vela", INVENTORY_SOURCE),
        ])?))?
        .activate()?;
    let folded_root = services.pin();
    let (folded, folded_turn) = run_active(services)?;
    assert_eq!(folded, complete);
    let effects_before_rollback = folded_turn.actor.event_calls;
    let restored = app.patches().rollback(rollback)?;
    assert_eq!(restored.generation_id(), complete_root.generation_id());
    assert_eq!(
        folded_turn.actor.event_calls, effects_before_rollback,
        "publication-only rollback must not retry or reverse host effects"
    );

    println!(
        "service_hard_switch_fixture rust={} rule={} delta1={} delta2={} snapshot={} \
         vela_methods=3 rollback={}->{}",
        rust.checksum,
        rule.checksum,
        reward.checksum,
        complete.checksum,
        folded.checksum,
        folded_root.generation_id().get(),
        restored.generation_id().get(),
    );
    Ok(())
}
