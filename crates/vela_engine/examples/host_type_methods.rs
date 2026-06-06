#![allow(clippy::result_large_err)]

use std::error::Error;
use std::path::PathBuf;

use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, TypeId};
use vela_engine::args::{HostArgType, TypedHostMut};
use vela_engine::engine::Engine;
use vela_engine::host_type::HostTypeSpec;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, FunctionAccess, TypeHint};
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, HostIndexCapability, MethodDesc, TypeDesc, TypeKey};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

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
const PLAYER_TRANSFER_TO_METHOD: HostMethodId = HostMethodId::new(300);
const PLAYER_TYPED_TRANSFER_METHOD: HostMethodId = HostMethodId::new(301);
const REWARD_GRANT_METHOD: HostMethodId = HostMethodId::new(400);

const PLAYER_OBJECT: HostObjectId = HostObjectId::new(10);
const SCORES_OBJECT: HostObjectId = HostObjectId::new(20);
const TAGS_OBJECT: HostObjectId = HostObjectId::new(30);
const REWARDS_OBJECT: HostObjectId = HostObjectId::new(40);

fn main() -> Result<(), Box<dyn Error>> {
    let engine = host_engine()?;
    let script = script_path();
    let program = engine.compile_file(&script)?;
    let mut runtime = Runtime::new(engine.clone(), program);
    let mut adapter = ExampleAdapter::new();
    let args = CallArgs::from_positional([
        OwnedValue::HostRef(host_ref(PLAYER_TYPE, PLAYER_OBJECT)),
        OwnedValue::HostRef(host_ref(INT_INT_MAP_TYPE, SCORES_OBJECT)),
        OwnedValue::HostRef(host_ref(TAG_SET_TYPE, TAGS_OBJECT)),
        OwnedValue::HostRef(host_ref(REWARD_SINK_TYPE, REWARDS_OBJECT)),
    ]);

    let output = runtime.call_with_adapter(
        "main",
        args,
        CallOptions::new(10_000, 1024 * 1024, 64),
        &mut adapter,
    )?;

    let mut typed_access = HostAccess::new();
    let mut typed_host = HostExecution {
        adapter: &mut adapter,
        access: &mut typed_access,
    };
    engine.call_native_method(
        PLAYER_TYPED_TRANSFER_METHOD,
        &HostPath::new(host_ref(PLAYER_TYPE, PLAYER_OBJECT)),
        &[
            OwnedValue::HostRef(host_ref(REWARD_SINK_TYPE, REWARDS_OBJECT)),
            OwnedValue::String("gem".to_owned()),
            OwnedValue::Int(4),
        ],
        &mut typed_host,
    )?;

    let final_count = adapter.read_path(&gold_count_path())?;
    let final_score = adapter.read_path(&score_path(1001))?;
    let reward_calls = adapter
        .method_calls()
        .iter()
        .filter(|(_, method, _)| *method == REWARD_GRANT_METHOD)
        .count();

    println!(
        "script_result={:?} final_count={final_count:?} score={final_score:?} \
         reward_calls={reward_calls}",
        output.value()
    );

    Ok(())
}

fn host_engine() -> Result<Engine, Box<dyn Error>> {
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

fn player_spec() -> HostTypeSpec {
    let owner = TypeKey::new(TypeId::new(1), "Player");
    HostTypeSpec::new(
        TypeDesc::new(owner.clone())
            .host_type(PLAYER_TYPE)
            .field(FieldDesc::new(PLAYER_INVENTORY_FIELD, "inventory").type_hint("Inventory"))
            .field(FieldDesc::new(PLAYER_REWARD_SINK_FIELD, "reward_sink").type_hint("RewardSink"))
            .method(MethodDesc::new(PLAYER_TRANSFER_TO_METHOD, "transfer_to")),
    )
    .typed_native_method_fn::<(TypedHostMut<RewardSinkArg>, String, i64), _>(
        NativeMethodDesc::new(owner, PLAYER_TYPED_TRANSFER_METHOD, "typed_transfer_to")
            .param("target", TypeHint::PathProxy)
            .param("item_id", TypeHint::String)
            .param("amount", TypeHint::Int)
            .returns(TypeHint::Null)
            .effects(EffectSet::host_write())
            .access(FunctionAccess::public()),
        typed_transfer_to_reward_sink,
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
    let owner = TypeKey::new(TypeId::new(4), "IntIntMap");
    HostTypeSpec::new(
        TypeDesc::new(owner)
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

struct RewardSinkArg;

impl HostArgType for RewardSinkArg {
    const TYPE_NAME: &'static str = "RewardSink";
    const HOST_TYPE_ID: Option<HostTypeId> = Some(REWARD_SINK_TYPE);
}

fn typed_transfer_to_reward_sink(
    _receiver: &HostPath,
    host: &mut HostExecution<'_>,
    target: TypedHostMut<RewardSinkArg>,
    item_id: String,
    amount: i64,
) -> VmResult<()> {
    host.access.call_method(
        host.adapter,
        target.into_path(),
        REWARD_GRANT_METHOD,
        vec![HostValue::String(item_id), HostValue::Int(amount)],
        None,
    )?;
    Ok(())
}

struct ExampleAdapter {
    inner: MockStateAdapter,
    method_calls: Vec<(HostPath, HostMethodId, Vec<HostValue>)>,
}

impl ExampleAdapter {
    fn new() -> Self {
        let mut inner = MockStateAdapter::new();
        inner.insert_value(gold_count_path(), HostValue::Int(3));
        inner.insert_value(tag_path("vip"), HostValue::Bool(true));
        inner.insert_object(host_ref(INT_INT_MAP_TYPE, SCORES_OBJECT));
        inner.insert_object(host_ref(REWARD_SINK_TYPE, REWARDS_OBJECT));
        Self {
            inner,
            method_calls: Vec::new(),
        }
    }

    fn method_calls(&self) -> &[(HostPath, HostMethodId, Vec<HostValue>)] {
        &self.method_calls
    }

    fn read_int(&self, path: &HostPath) -> HostResult<i64> {
        match self.inner.read_path(path)? {
            HostValue::Int(value) => Ok(value),
            _ => Err(host_error(HostErrorKind::MissingPath {
                path: path.clone(),
            })),
        }
    }

    fn read_bool(&self, path: &HostPath) -> HostResult<bool> {
        match self.inner.read_path(path)? {
            HostValue::Bool(value) => Ok(value),
            _ => Err(host_error(HostErrorKind::MissingPath {
                path: path.clone(),
            })),
        }
    }
}

impl ScriptStateAdapter for ExampleAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        self.inner.read_path(path)
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        self.inner.write_path(path, value)
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        self.inner.remove_path(path)
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        self.method_calls
            .push((path.clone(), method, args.to_vec()));
        match method {
            MAP_GET_METHOD => {
                let key = expect_int_arg(args, 0)?;
                self.inner.read_path(&path.clone().key(key.to_string()))
            }
            MAP_SET_METHOD => {
                let key = expect_int_arg(args, 0)?;
                let value = expect_int_arg(args, 1)?;
                self.inner
                    .write_path(&path.clone().key(key.to_string()), HostValue::Int(value))?;
                Ok(HostValue::Null)
            }
            MAP_ADD_TO_METHOD => {
                let key = expect_int_arg(args, 0)?;
                let amount = expect_int_arg(args, 1)?;
                let item_path = path.clone().key(key.to_string());
                let current = self.read_int(&item_path)?;
                self.inner
                    .write_path(&item_path, HostValue::Int(current + amount))?;
                Ok(HostValue::Null)
            }
            MAP_CONTAINS_METHOD => {
                let key = expect_int_arg(args, 0)?;
                Ok(HostValue::Bool(
                    self.inner
                        .read_path(&path.clone().key(key.to_string()))
                        .is_ok(),
                ))
            }
            TAG_CONTAINS_METHOD => {
                let tag = expect_string_arg(args, 0)?;
                self.read_bool(&path.clone().key(tag)).map(HostValue::Bool)
            }
            PLAYER_TRANSFER_TO_METHOD => {
                let target = expect_host_ref_arg(args, 0)?;
                let item_id = expect_string_arg(args, 1)?.to_owned();
                let amount = expect_int_arg(args, 2)?;
                self.method_calls.push((
                    HostPath::new(target),
                    REWARD_GRANT_METHOD,
                    vec![HostValue::String(item_id), HostValue::Int(amount)],
                ));
                Ok(HostValue::Null)
            }
            REWARD_GRANT_METHOD => Ok(HostValue::Null),
            _ => Err(host_error(HostErrorKind::UnsupportedMethod { method })),
        }
    }
}

fn expect_int_arg(args: &[HostValue], index: usize) -> HostResult<i64> {
    match args.get(index) {
        Some(HostValue::Int(value)) => Ok(*value),
        _ => Err(host_error(HostErrorKind::InvalidArgument {
            expected: "int argument",
        })),
    }
}

fn expect_string_arg(args: &[HostValue], index: usize) -> HostResult<&str> {
    match args.get(index) {
        Some(HostValue::String(value)) => Ok(value),
        _ => Err(host_error(HostErrorKind::InvalidArgument {
            expected: "string argument",
        })),
    }
}

fn expect_host_ref_arg(args: &[HostValue], index: usize) -> HostResult<HostRef> {
    match args.get(index) {
        Some(HostValue::HostRef(value)) => Ok(*value),
        _ => Err(host_error(HostErrorKind::InvalidArgument {
            expected: "host ref argument",
        })),
    }
}

fn gold_count_path() -> HostPath {
    HostPath::new(host_ref(PLAYER_TYPE, PLAYER_OBJECT))
        .field(PLAYER_INVENTORY_FIELD)
        .field(INVENTORY_ITEMS_FIELD)
        .key("gold")
        .field(ITEM_STACK_COUNT_FIELD)
}

fn score_path(key: i64) -> HostPath {
    HostPath::new(host_ref(INT_INT_MAP_TYPE, SCORES_OBJECT)).key(key.to_string())
}

fn tag_path(tag: &str) -> HostPath {
    HostPath::new(host_ref(TAG_SET_TYPE, TAGS_OBJECT)).key(tag)
}

fn host_ref(type_id: HostTypeId, object_id: HostObjectId) -> HostRef {
    HostRef::new(type_id, object_id, 1)
}

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/host_type_methods/handle.vela")
}

fn host_error(kind: HostErrorKind) -> HostError {
    HostError {
        kind,
        source_span: None,
    }
}
