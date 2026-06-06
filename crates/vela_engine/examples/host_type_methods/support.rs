use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::PathBuf;

use vela_engine::prelude::*;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::path::PathSegment;

const PLAYER_TYPE: HostTypeId = HostTypeId::new(1);
const INVENTORY_TYPE: HostTypeId = HostTypeId::new(2);
const ITEM_STACK_TYPE: HostTypeId = HostTypeId::new(3);
const INT_INT_MAP_TYPE: HostTypeId = HostTypeId::new(4);
const TAG_SET_TYPE: HostTypeId = HostTypeId::new(5);
const REWARD_SINK_TYPE: HostTypeId = HostTypeId::new(6);

const PLAYER_INVENTORY_FIELD: FieldId = FieldId::new(10);
const PLAYER_REWARD_SINK_FIELD: FieldId = FieldId::new(11);
const INVENTORY_ITEMS_FIELD: FieldId = FieldId::new(20);
const ITEM_STACK_COUNT_FIELD: FieldId = FieldId::new(30);

const MAP_GET_METHOD: HostMethodId = HostMethodId::new(100);
const MAP_SET_METHOD: HostMethodId = HostMethodId::new(101);
const MAP_ADD_TO_METHOD: HostMethodId = HostMethodId::new(102);
const MAP_CONTAINS_METHOD: HostMethodId = HostMethodId::new(103);
const TAG_CONTAINS_METHOD: HostMethodId = HostMethodId::new(200);
const REWARD_GRANT_METHOD: HostMethodId = HostMethodId::new(400);

pub(crate) fn host_engine() -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_host_type_spec(player_spec())
        .register_host_type_spec(inventory_spec())
        .register_host_type_spec(item_stack_spec())
        .register_host_type_spec(int_int_map_spec())
        .register_host_type_spec(tag_set_spec())
        .register_host_type_spec(reward_sink_spec())
        .build()?)
}

pub(crate) fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/host_type_methods/handle.vela")
}

#[derive(Debug)]
pub(crate) struct Player {
    inventory: Inventory,
    reward_sink: RewardSink,
}

impl Player {
    pub(crate) fn new() -> Self {
        let mut inventory = Inventory::default();
        inventory
            .items
            .insert("gold".to_owned(), ItemStack { count: 3 });
        Self {
            inventory,
            reward_sink: RewardSink::default(),
        }
    }

    pub(crate) fn gold_count(&self) -> i64 {
        self.inventory
            .items
            .get("gold")
            .map(|stack| stack.count)
            .unwrap_or_default()
    }

    pub(crate) fn reward_sink_grant_count(&self) -> usize {
        self.reward_sink.grant_count()
    }
}

#[derive(Debug, Default)]
struct Inventory {
    items: BTreeMap<String, ItemStack>,
}

#[derive(Debug, Default)]
struct ItemStack {
    count: i64,
}

#[derive(Debug, Default)]
pub(crate) struct IntIntMap {
    values: BTreeMap<i64, i64>,
}

impl IntIntMap {
    pub(crate) fn get(&self, key: i64) -> Option<i64> {
        self.values.get(&key).copied()
    }
}

#[derive(Debug, Default)]
pub(crate) struct TagSet {
    values: BTreeSet<String>,
}

impl<const N: usize> From<[&str; N]> for TagSet {
    fn from(values: [&str; N]) -> Self {
        Self {
            values: values.into_iter().map(str::to_owned).collect(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct RewardSink {
    grants: Vec<(String, i64)>,
}

impl RewardSink {
    pub(crate) fn grant_count(&self) -> usize {
        self.grants.len()
    }

    fn grant(&mut self, item_id: String, amount: i64) {
        self.grants.push((item_id, amount));
    }
}

impl ScriptHostObject for Player {
    fn host_type_id(&self) -> HostTypeId {
        PLAYER_TYPE
    }

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue> {
        match path.segments.as_slice() {
            [
                PathSegment::Field(PLAYER_INVENTORY_FIELD),
                PathSegment::Field(INVENTORY_ITEMS_FIELD),
                PathSegment::Key(item_id),
                PathSegment::Field(ITEM_STACK_COUNT_FIELD),
            ] => self
                .inventory
                .items
                .get(item_id)
                .map(|stack| HostValue::Int(stack.count))
                .ok_or_else(|| missing_path(path)),
            _ => Err(missing_path(path)),
        }
    }

    fn write_host_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        match path.segments.as_slice() {
            [
                PathSegment::Field(PLAYER_INVENTORY_FIELD),
                PathSegment::Field(INVENTORY_ITEMS_FIELD),
                PathSegment::Key(item_id),
                PathSegment::Field(ITEM_STACK_COUNT_FIELD),
            ] => {
                let count = expect_int_value(value)?;
                self.inventory
                    .items
                    .entry(item_id.clone())
                    .or_default()
                    .count = count;
                Ok(())
            }
            _ => Err(missing_path(path)),
        }
    }

    fn call_host_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match (path.segments.as_slice(), method) {
            ([PathSegment::Field(PLAYER_REWARD_SINK_FIELD)], REWARD_GRANT_METHOD) => {
                self.reward_sink.grant(
                    expect_string_arg(args, 0)?.to_owned(),
                    expect_int_arg(args, 1)?,
                );
                Ok(HostValue::Null)
            }
            _ => Err(unsupported_method(method)),
        }
    }
}

impl ScriptHostObject for IntIntMap {
    fn host_type_id(&self) -> HostTypeId {
        INT_INT_MAP_TYPE
    }

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue> {
        match path.segments.as_slice() {
            [PathSegment::Key(key)] => self
                .get(parse_i64_key(key)?)
                .map(HostValue::Int)
                .ok_or_else(|| missing_path(path)),
            _ => Err(missing_path(path)),
        }
    }

    fn write_host_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        match path.segments.as_slice() {
            [PathSegment::Key(key)] => {
                self.values
                    .insert(parse_i64_key(key)?, expect_int_value(value)?);
                Ok(())
            }
            _ => Err(missing_path(path)),
        }
    }

    fn call_host_method(
        &mut self,
        _path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match method {
            MAP_GET_METHOD => self
                .get(expect_int_arg(args, 0)?)
                .map(HostValue::Int)
                .ok_or_else(|| invalid_arg("existing map key")),
            MAP_SET_METHOD => {
                self.values
                    .insert(expect_int_arg(args, 0)?, expect_int_arg(args, 1)?);
                Ok(HostValue::Null)
            }
            MAP_ADD_TO_METHOD => {
                let key = expect_int_arg(args, 0)?;
                let amount = expect_int_arg(args, 1)?;
                *self.values.entry(key).or_default() += amount;
                Ok(HostValue::Null)
            }
            MAP_CONTAINS_METHOD => Ok(HostValue::Bool(
                self.values.contains_key(&expect_int_arg(args, 0)?),
            )),
            _ => Err(unsupported_method(method)),
        }
    }
}

impl ScriptHostObject for TagSet {
    fn host_type_id(&self) -> HostTypeId {
        TAG_SET_TYPE
    }

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue> {
        match path.segments.as_slice() {
            [PathSegment::Key(value)] => Ok(HostValue::Bool(self.values.contains(value))),
            _ => Err(missing_path(path)),
        }
    }

    fn call_host_method(
        &mut self,
        _path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match method {
            TAG_CONTAINS_METHOD => Ok(HostValue::Bool(
                self.values.contains(expect_string_arg(args, 0)?),
            )),
            _ => Err(unsupported_method(method)),
        }
    }
}

impl ScriptHostObject for RewardSink {
    fn host_type_id(&self) -> HostTypeId {
        REWARD_SINK_TYPE
    }

    fn read_host_path(&self, path: &HostPath) -> HostResult<HostValue> {
        Err(missing_path(path))
    }

    fn call_host_method(
        &mut self,
        _path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match method {
            REWARD_GRANT_METHOD => {
                self.grant(
                    expect_string_arg(args, 0)?.to_owned(),
                    expect_int_arg(args, 1)?,
                );
                Ok(HostValue::Null)
            }
            _ => Err(unsupported_method(method)),
        }
    }
}

fn player_spec() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .host_type(PLAYER_TYPE)
            .field(FieldDesc::new(PLAYER_INVENTORY_FIELD, "inventory").type_hint("Inventory"))
            .field(FieldDesc::new(PLAYER_REWARD_SINK_FIELD, "reward_sink").type_hint("RewardSink")),
    )
}

fn inventory_spec() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory"))
            .host_type(INVENTORY_TYPE)
            .field(FieldDesc::new(INVENTORY_ITEMS_FIELD, "items").type_hint("IntItemMap")),
    )
}

fn item_stack_spec() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(3), "ItemStack"))
            .host_type(ITEM_STACK_TYPE)
            .field(
                FieldDesc::new(ITEM_STACK_COUNT_FIELD, "count")
                    .type_hint("int")
                    .writable(true),
            ),
    )
}

fn int_int_map_spec() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(4), "IntIntMap"))
            .host_type(INT_INT_MAP_TYPE)
            .index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .writable(true)
                    .addable(true)
                    .removable(true)
                    .key_type("int")
                    .value_type("int"),
            )
            .method(MethodDesc::new(MAP_GET_METHOD, "get"))
            .method(MethodDesc::new(MAP_SET_METHOD, "set"))
            .method(MethodDesc::new(MAP_ADD_TO_METHOD, "add_to"))
            .method(MethodDesc::new(MAP_CONTAINS_METHOD, "contains")),
    )
}

fn tag_set_spec() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(5), "TagSet"))
            .host_type(TAG_SET_TYPE)
            .method(MethodDesc::new(TAG_CONTAINS_METHOD, "contains")),
    )
}

fn reward_sink_spec() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(6), "RewardSink"))
            .host_type(REWARD_SINK_TYPE)
            .method(MethodDesc::new(REWARD_GRANT_METHOD, "grant")),
    )
}

fn expect_int_value(value: HostValue) -> HostResult<i64> {
    match value {
        HostValue::Int(value) => Ok(value),
        _ => Err(invalid_arg("int value")),
    }
}

fn expect_int_arg(args: &[HostValue], index: usize) -> HostResult<i64> {
    match args.get(index) {
        Some(HostValue::Int(value)) => Ok(*value),
        _ => Err(invalid_arg("int argument")),
    }
}

fn expect_string_arg(args: &[HostValue], index: usize) -> HostResult<&str> {
    match args.get(index) {
        Some(HostValue::String(value)) => Ok(value),
        _ => Err(invalid_arg("string argument")),
    }
}

fn parse_i64_key(key: &str) -> HostResult<i64> {
    key.parse()
        .map_err(|_| invalid_arg("integer string host key"))
}

fn invalid_arg(expected: &'static str) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidArgument { expected },
        source_span: None,
    }
}

fn missing_path(path: &HostPath) -> HostError {
    HostError {
        kind: HostErrorKind::MissingPath { path: path.clone() },
        source_span: None,
    }
}

fn unsupported_method(method: HostMethodId) -> HostError {
    HostError {
        kind: HostErrorKind::UnsupportedMethod { method },
        source_span: None,
    }
}
