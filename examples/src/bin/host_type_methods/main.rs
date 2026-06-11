#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use vela_engine::prelude::*;
use vela_examples::example_file;
use vela_macros::{ScriptHost, script_methods};

fn main() -> Result<(), Box<dyn Error>> {
    let engine = Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_script_host::<Player>()
        .register_host_type::<Inventory>()
        .register_host_type::<ItemStack>()
        .register_host_type_spec(string_item_map_type())
        .register_script_host::<IntIntMap>()
        .register_script_host::<TagSet>()
        .register_script_host::<RewardSink>()
        .build()?;
    let program = engine.compile_file(script_path())?;
    let mut runtime = Runtime::new(engine, program);

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
#[script(path = "examples::host_type_methods::Player")]
struct Player {
    #[script(get, hint = "Inventory")]
    inventory: Inventory,
    #[script(get, hint = "RewardSink")]
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

#[script_methods]
impl Player {}

#[derive(Debug, Default, ScriptHost)]
#[script(path = "examples::host_type_methods::Inventory")]
struct Inventory {
    #[script(get, hint = "StringItemMap")]
    items: BTreeMap<String, ItemStack>,
}

#[script_methods]
impl Inventory {}

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
#[script(path = "examples::host_type_methods::ItemStack")]
struct ItemStack {
    #[script(get, set, hint = "i64")]
    count: i64,
}

#[derive(Debug, Default, ScriptHost)]
#[script(path = "examples::host_type_methods::IntIntMap")]
struct IntIntMap {
    #[script(skip)]
    values: BTreeMap<i64, i64>,
}

impl IntIntMap {
    fn value(&self, key: i64) -> i64 {
        self.values.get(&key).copied().unwrap_or_default()
    }
}

#[script_methods]
impl IntIntMap {
    #[script_method(effect = "read_host")]
    fn get(&self, key: i64) -> i64 {
        self.value(key)
    }

    #[script_method(effect = "write_host")]
    fn set(&mut self, key: i64, value: i64) {
        self.values.insert(key, value);
    }

    #[script_method(effect = "write_host")]
    fn add_to(&mut self, key: i64, amount: i64) {
        *self.values.entry(key).or_default() += amount;
    }

    #[script_method(effect = "read_host")]
    fn contains(&self, key: i64) -> bool {
        self.values.contains_key(&key)
    }
}

#[derive(Debug, Default, ScriptHost)]
#[script(path = "examples::host_type_methods::TagSet")]
struct TagSet {
    #[script(skip)]
    values: BTreeSet<String>,
}

impl<const N: usize> From<[&str; N]> for TagSet {
    fn from(values: [&str; N]) -> Self {
        Self {
            values: values.into_iter().map(str::to_owned).collect(),
        }
    }
}

#[script_methods]
impl TagSet {
    #[script_method(effect = "read_host")]
    fn contains(&self, value: String) -> bool {
        self.values.contains(&value)
    }
}

#[derive(Debug, Default, ScriptHost)]
#[script(path = "examples::host_type_methods::RewardSink")]
struct RewardSink {
    #[script(skip)]
    grants: Vec<(String, i64)>,
}

impl RewardSink {
    fn grant_count(&self) -> usize {
        self.grants.len()
    }
}

#[script_methods]
impl RewardSink {
    #[script_method(effect = "write_host")]
    fn grant(&mut self, item_id: String, amount: i64) {
        self.grants.push((item_id, amount));
    }
}

fn script_path() -> std::path::PathBuf {
    example_file("host_type_methods", "handle.vela")
}
