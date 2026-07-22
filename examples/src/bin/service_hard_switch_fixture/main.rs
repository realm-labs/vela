use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

type ServiceResult<T> = Result<T, ServiceError>;
type ServiceFuture<'call, T> = Pin<Box<dyn Future<Output = T> + Send + 'call>>;

#[derive(Debug)]
struct ServiceError {
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

#[derive(Clone, Debug)]
struct ItemGrant {
    template_id: i32,
    count: i32,
    tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
struct DisplayItem {
    template_id: i32,
    count: i32,
    label: String,
}

#[derive(Debug)]
struct GrantRequest {
    items: Vec<ItemGrant>,
    multipliers: BTreeMap<i32, i32>,
}

#[derive(Debug)]
struct GrantResponse {
    granted: Vec<DisplayItem>,
}

#[derive(Debug)]
struct HostActor {
    item_counts: BTreeMap<i32, i32>,
    last_reward_count: usize,
}

struct HostTurn {
    actor: HostActor,
    services: Arc<RustServices>,
}

trait RewardService: Send + Sync {
    fn apply(
        &self,
        actor: &mut HostActor,
        grouped: &BTreeMap<i32, i32>,
        labels: &BTreeMap<i32, String>,
    ) -> ServiceResult<Vec<DisplayItem>>;
}

trait InventoryService: Send + Sync {
    fn grant(
        &self,
        turn: &mut HostTurn,
        items: &[ItemGrant],
        multipliers: &BTreeMap<i32, i32>,
    ) -> ServiceResult<Vec<DisplayItem>>;

    fn current_count(&self, turn: &HostTurn, template_id: i32) -> i32;
}

trait GrantHandlerService: Send + Sync {
    fn handle<'call>(
        &'call self,
        turn: &'call mut HostTurn,
        request: GrantRequest,
    ) -> ServiceFuture<'call, ServiceResult<GrantResponse>>;
}

struct RustRewardService;

impl RewardService for RustRewardService {
    fn apply(
        &self,
        actor: &mut HostActor,
        grouped: &BTreeMap<i32, i32>,
        labels: &BTreeMap<i32, String>,
    ) -> ServiceResult<Vec<DisplayItem>> {
        let mut granted = Vec::with_capacity(grouped.len());
        for (&template_id, &count) in grouped {
            if count <= 0 {
                return Err(ServiceError::new("reward count must be positive"));
            }
            *actor.item_counts.entry(template_id).or_default() += count;
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
        for item in items {
            let multiplier = multipliers.get(&item.template_id).copied().unwrap_or(1);
            *grouped.entry(item.template_id).or_default() += item.count * multiplier;
            if let Some(label) = item.tags.get("label") {
                labels
                    .entry(item.template_id)
                    .or_insert_with(|| label.clone());
            }
        }

        let reward = Arc::clone(&turn.services.reward);
        let granted = reward.apply(&mut turn.actor, &grouped, &labels)?;
        turn.actor.last_reward_count = granted.len();
        Ok(granted)
    }

    fn current_count(&self, turn: &HostTurn, template_id: i32) -> i32 {
        turn.actor
            .item_counts
            .get(&template_id)
            .copied()
            .unwrap_or_default()
    }
}

struct RustGrantHandlerService;

impl GrantHandlerService for RustGrantHandlerService {
    fn handle<'call>(
        &'call self,
        turn: &'call mut HostTurn,
        request: GrantRequest,
    ) -> ServiceFuture<'call, ServiceResult<GrantResponse>> {
        Box::pin(async move {
            std::future::ready(()).await;
            let inventory = Arc::clone(&turn.services.inventory);
            let granted = inventory.grant(turn, &request.items, &request.multipliers)?;
            Ok(GrantResponse { granted })
        })
    }
}

struct RustServices {
    inventory: Arc<dyn InventoryService>,
    reward: Arc<dyn RewardService>,
    handler: Arc<dyn GrantHandlerService>,
}

impl RustServices {
    fn defaults() -> Arc<Self> {
        Arc::new(Self {
            inventory: Arc::new(RustInventoryService),
            reward: Arc::new(RustRewardService),
            handler: Arc::new(RustGrantHandlerService),
        })
    }
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
    let services = RustServices::defaults();
    let mut turn = HostTurn {
        actor: HostActor {
            item_counts: BTreeMap::new(),
            last_reward_count: 0,
        },
        services: Arc::clone(&services),
    };
    let request = GrantRequest {
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
    };

    let handler = Arc::clone(&services.handler);
    let response = block_on(handler.handle(&mut turn, request))?;
    let count = services.inventory.current_count(&turn, 7);
    let checksum = response.granted.iter().fold(0_i64, |checksum, item| {
        checksum
            + i64::from(item.template_id) * 100
            + i64::from(item.count) * 10
            + i64::try_from(item.label.len()).unwrap_or_default()
    });
    println!(
        "service_hard_switch_fixture granted={} item7={} last_reward_count={} checksum={checksum}",
        response.granted.len(),
        count,
        turn.actor.last_reward_count,
    );
    Ok(())
}
