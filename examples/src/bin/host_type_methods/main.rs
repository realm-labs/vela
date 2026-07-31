#![cfg_attr(not(test), deny(clippy::wildcard_imports))]
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use vela_def::TypeId;
use vela_engine::host_type::HostTypeSpec;
use vela_engine::prelude::*;
use vela_macros::{ScriptHost, methods};
use vela_reflect::registry::{HostIndexCapability, TypeDesc, TypeKey};

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_type::<Player>()
        .register_type::<Inventory>()
        .register_type::<ItemStack>()
        .register_type_spec(string_item_map_type())
        .register_type::<IntIntMap>()
        .register_type::<TagSet>()
        .register_type::<RewardSink>()
        .register_exports(Player::vela_inherent_exports())
        .register_exports(IntIntMap::vela_inherent_exports())
        .register_exports(TagSet::vela_inherent_exports())
        .register_exports(RewardSink::vela_inherent_exports())
        .build()?;
    let program = engine.compile_source(include_str!("handle.vela"))?;
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let mut player = Player::new();
    let mut scores = IntIntMap::default();
    let mut tags = TagSet::from(["vip"]);
    let mut rewards = RewardSink::default();

    let output = runtime.call(
        "main",
        CallArgs::new()
            .with_host_mut("player", &mut player)
            .with_host_mut("scores", &mut scores)
            .with_host_mut("tags", &mut tags)
            .with_host_mut("rewards", &mut rewards),
        CallOptions::new(10_000, 1024 * 1024, 64),
    )?;

    println!(
        "script_result={:?} final_count={} score={} reward_calls={}",
        runtime.value_to_owned(&output)?,
        player.gold_count(),
        scores.value(1001),
        rewards.grant_count() + player.reward_sink_grant_count()
    );

    Ok(())
}

#[derive(Debug, ScriptHost)]
#[vela(path = "examples::host_type_methods::Player")]
struct Player {
    #[vela(get, hint = "Inventory")]
    inventory: Inventory,
    #[vela(get, hint = "RewardSink")]
    reward_sink: RewardSink,
}

impl Player {
    fn new() -> Self {
        let mut inventory = Inventory::default();
        inventory
            .items
            .insert("gold".to_owned(), ItemStack { count: 3 });
        Self {
            inventory,
            reward_sink: RewardSink::default(),
        }
    }

    fn gold_count(&self) -> i64 {
        self.inventory
            .items
            .get("gold")
            .map(|stack| stack.count)
            .unwrap_or_default()
    }

    fn reward_sink_grant_count(&self) -> usize {
        self.reward_sink.grant_count()
    }
}

#[methods]
impl Player {
    pub fn reward_sink_mut(&mut self) -> &mut RewardSink {
        &mut self.reward_sink
    }
}

#[derive(Debug, Default, ScriptHost)]
#[vela(path = "examples::host_type_methods::Inventory")]
struct Inventory {
    #[vela(get, hint = "StringItemMap")]
    items: BTreeMap<String, ItemStack>,
}

fn string_item_map_type() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(8_801), "StringItemMap")).index_capability(
            HostIndexCapability::new()
                .readable(true)
                .writable(true)
                .key_type("string")
                .value_type("ItemStack"),
        ),
    )
}

#[derive(Debug, Default, ScriptHost)]
#[vela(path = "examples::host_type_methods::ItemStack")]
struct ItemStack {
    #[vela(get, set, hint = "i64")]
    count: i64,
}

#[derive(Debug, Default, ScriptHost)]
#[vela(path = "examples::host_type_methods::IntIntMap")]
struct IntIntMap {
    #[vela(skip)]
    values: BTreeMap<i64, i64>,
}

impl IntIntMap {
    fn value(&self, key: i64) -> i64 {
        self.values.get(&key).copied().unwrap_or_default()
    }
}

#[methods]
impl IntIntMap {
    pub fn get(&self, key: i64) -> i64 {
        self.value(key)
    }

    pub fn set(&mut self, key: i64, value: i64) {
        self.values.insert(key, value);
    }

    pub fn add_to(&mut self, key: i64, amount: i64) {
        *self.values.entry(key).or_default() += amount;
    }

    pub fn contains(&self, key: i64) -> bool {
        self.values.contains_key(&key)
    }
}

#[derive(Debug, Default, ScriptHost)]
#[vela(path = "examples::host_type_methods::TagSet")]
struct TagSet {
    #[vela(skip)]
    values: BTreeSet<String>,
}

impl<const N: usize> From<[&str; N]> for TagSet {
    fn from(values: [&str; N]) -> Self {
        Self {
            values: values.into_iter().map(str::to_owned).collect(),
        }
    }
}

#[methods]
impl TagSet {
    pub fn contains(&self, value: String) -> bool {
        self.values.contains(&value)
    }
}

#[derive(Debug, Default, ScriptHost)]
#[vela(path = "examples::host_type_methods::RewardSink")]
struct RewardSink {
    #[vela(skip)]
    grants: Vec<(String, i64)>,
}

impl RewardSink {
    fn grant_count(&self) -> usize {
        self.grants.len()
    }
}

#[methods]
impl RewardSink {
    pub fn grant(&mut self, item_id: String, amount: i64) {
        self.grants.push((item_id, amount));
    }
}
