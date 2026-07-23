use vela_analysis::registry::RegistryFacts;
use vela_common::{
    CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation, HostMethodId,
    HostTypeId, InteropTypeId, ReceiverCapabilities, ReceiverCapability, SourceId, StoragePolicy,
};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_host::error::HostResult;
use vela_host::object::ScriptHostObject;
use vela_host::target::HostTargetInstance;
use vela_host::value::HostValue;
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::args::FromScriptArg;
use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::method::NativeMethodDesc;
use crate::native::{EffectSet, TypeHint};
use crate::runtime::{CallArgs, CallOptions, Runtime};
use crate::type_binding::{TypeBinding, ValueCodec};

struct ExternalHost {
    amount: i64,
}

impl ScriptHostObject for ExternalHost {
    fn host_type_id(&self) -> HostTypeId {
        HostTypeId::new(201)
    }

    fn read_resolved_host(
        &self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
    ) -> HostResult<HostValue> {
        Ok(HostValue::Scalar(vela_common::ScalarValue::I64(
            self.amount,
        )))
    }

    fn write_resolved_host(
        &mut self,
        _access: vela_host::resolved::ResolvedHostAccess,
        _target: HostTargetInstance<'_>,
        value: HostValue,
    ) -> HostResult<()> {
        let HostValue::Scalar(vela_common::ScalarValue::I64(amount)) = value else {
            return Err(vela_host::error::HostError {
                kind: vela_host::error::HostErrorKind::InvalidArgument {
                    expected: "i64 ExternalHost value",
                },
                source_span: None,
            });
        };
        self.amount = amount;
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ExternalValue {
    amount: i64,
}

fn host_desc(id: u128, host_id: u64) -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(id), "host::ExternalHost"))
        .host_type(HostTypeId::new(host_id))
        .field(
            FieldDesc::new(FieldId::new(11), "value")
                .type_hint("i64")
                .writable(true),
        )
}

fn value_desc(id: u128, field_name: &str) -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(id), "host::ExternalValue"))
        .kind(TypeKind::ScriptStruct)
        .field(FieldDesc::new(FieldId::new(21), field_name).type_hint("i64"))
}

fn external_value_binding(desc: TypeDesc) -> TypeBinding<ExternalValue> {
    TypeBinding::value_with_codec(
        desc,
        ValueCodec::new(encode_external_value, decode_external_value),
    )
}

fn external_value_constructor(param_hint: TypeHint) -> crate::native::NativeFunctionDesc {
    crate::native::NativeFunctionDesc::new("host::ExternalValue::new", FunctionId::new(401))
        .param("amount", param_hint)
        .returns(TypeHint::Record(value_desc(102, "amount").key))
        .effects(EffectSet::pure())
}

fn construct_external_value(
    args: &[OwnedValue],
    _host: &mut vela_vm::HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let [amount] = args else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host::ExternalValue::new arguments",
        }));
    };
    Ok(encode_external_value(ExternalValue {
        amount: i64::from_script_arg(amount)?,
    }))
}

fn external_host_constructor() -> crate::native::NativeFunctionDesc {
    crate::native::NativeFunctionDesc::new("host::ExternalHost::new", FunctionId::new(411))
        .param("amount", TypeHint::i64())
        .returns(TypeHint::Host(host_desc(101, 201).key))
        .effects(EffectSet::pure())
}

fn construct_external_host(
    args: &[OwnedValue],
    _host: &mut vela_vm::HostExecution<'_>,
) -> VmResult<ExternalHost> {
    let [amount] = args else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host::ExternalHost::new arguments",
        }));
    };
    Ok(ExternalHost {
        amount: i64::from_script_arg(amount)?,
    })
}

fn encode_external_value(value: ExternalValue) -> OwnedValue {
    OwnedValue::record(
        "host::ExternalValue",
        [("amount", OwnedValue::from(value.amount))],
    )
}

fn decode_external_value(value: &OwnedValue) -> VmResult<ExternalValue> {
    let OwnedValue::Record { type_name, fields } = value else {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host::ExternalValue codec",
        }));
    };
    if type_name != "host::ExternalValue" || fields.len() != 1 {
        return Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "host::ExternalValue codec",
        }));
    }
    Ok(ExternalValue {
        amount: i64::from_script_arg(fields.get("amount").ok_or_else(|| {
            VmError::new(VmErrorKind::TypeMismatch {
                operation: "host::ExternalValue codec",
            })
        })?)?,
    })
}

#[test]
fn unified_type_binding_is_sealed_into_engine_and_reflection_registry() {
    let owner = host_desc(101, 201).key;
    let engine = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201)).method_desc(
                NativeMethodDesc::new(owner.clone(), HostMethodId::new(301), "read")
                    .receiver(ReceiverCapability::Shared)
                    .returns(TypeHint::i64())
                    .effects(EffectSet::host_read()),
            ),
        )
        .register_rust_type::<ExternalValue>(external_value_binding(value_desc(102, "amount")))
        .build()
        .expect("unified type bindings should seal");

    let bindings = engine.type_bindings();
    let host = bindings
        .get_for::<ExternalHost>()
        .expect("Rust host type binding");
    assert_eq!(host.id, InteropTypeId::new(101));
    assert_eq!(host.storage, StoragePolicy::Host);
    assert!(host.capabilities.contains(ReceiverCapability::Owned));
    assert!(host.capabilities.contains(ReceiverCapability::Shared));
    assert!(host.capabilities.contains(ReceiverCapability::Exclusive));
    assert!(!host.capabilities.contains(ReceiverCapability::Construct));

    let reflected_registry = engine.registry();
    assert_eq!(
        reflected_registry
            .type_binding_for_key(&owner)
            .expect("reflection uses the sealed binding facts"),
        host
    );
    assert_eq!(
        reflected_registry
            .type_binding_snapshot()
            .expect("sealed binding snapshot")
            .checksum(),
        bindings.checksum()
    );
    assert!(
        reflected_registry
            .type_by_name("host::ExternalHost")
            .expect("type metadata")
            .methods
            .iter()
            .any(|method| method.name == "read")
    );

    let value = bindings
        .get_for::<ExternalValue>()
        .expect("Rust value type binding");
    assert_eq!(value.storage, StoragePolicy::Value);
    assert_eq!(reflected_registry.type_bindings().count(), 2);

    let compiler_facts = RegistryFacts::from_compile_view(engine.compiler_registry())
        .expect("compiler binding facts");
    assert_eq!(
        compiler_facts
            .type_binding_fact("host::ExternalHost")
            .expect("compiler consumes the same binding")
            .abi_fingerprint,
        host.abi_fingerprint
    );
    assert_eq!(
        compiler_facts.type_binding_checksum(),
        Some(bindings.checksum())
    );
    assert_eq!(
        compiler_facts
            .method_access_fact("host::ExternalHost", "read")
            .expect("compiler method receiver fact")
            .receiver,
        ReceiverCapability::Shared
    );
}

#[test]
fn type_binding_fingerprint_ignores_docs_but_tracks_structural_abi() {
    let first = Engine::builder()
        .register_rust_type::<ExternalValue>(external_value_binding(
            value_desc(102, "amount").docs("first docs"),
        ))
        .build()
        .expect("first binding");
    let second = Engine::builder()
        .register_rust_type::<ExternalValue>(external_value_binding(
            value_desc(102, "amount").docs("different docs"),
        ))
        .build()
        .expect("second binding");
    let changed = Engine::builder()
        .register_rust_type::<ExternalValue>(external_value_binding(value_desc(102, "total")))
        .build()
        .expect("changed binding");

    let first = first.type_bindings();
    let second = second.type_bindings();
    let changed = changed.type_bindings();
    assert_eq!(
        first
            .get_for::<ExternalValue>()
            .expect("first value binding")
            .abi_fingerprint,
        second
            .get_for::<ExternalValue>()
            .expect("second value binding")
            .abi_fingerprint
    );
    assert_eq!(first.checksum(), second.checksum());
    assert_ne!(
        first
            .get_for::<ExternalValue>()
            .expect("first value binding")
            .abi_fingerprint,
        changed
            .get_for::<ExternalValue>()
            .expect("changed value binding")
            .abi_fingerprint
    );
    assert_ne!(first.checksum(), changed.checksum());
}

#[test]
fn type_binding_fingerprint_tracks_collection_view_capabilities() {
    let desc = value_desc(102, "amount");
    let plain = external_value_binding(desc.clone()).abi_fingerprint();
    let viewed = external_value_binding(desc)
        .collection_view_capabilities(CollectionViewCapabilities::mutable(
            CollectionViewKind::Array,
            CollectionViewMutation::Growable,
        ))
        .abi_fingerprint();

    assert_ne!(plain, viewed);
}

#[test]
fn collection_view_kind_must_match_the_bound_representation() {
    let result = Engine::builder()
        .register_rust_type::<ExternalValue>(
            external_value_binding(value_desc(102, "amount")).collection_view_capabilities(
                CollectionViewCapabilities::read_only(CollectionViewKind::Array),
            ),
        )
        .build();

    assert!(matches!(
        result,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingCollectionView {
                name: "host::ExternalValue".to_owned(),
                reason: "view kind does not match the registered value kind",
            }
    ));
}

#[test]
fn bytes_binding_accepts_array_view_capabilities() {
    let binding = TypeBinding::<Vec<u8>>::value(
        TypeDesc::new(TypeKey::new(TypeId::new(103), "host::Bytes")).kind(TypeKind::Bytes),
    )
    .collection_view_capabilities(CollectionViewCapabilities::mutable(
        CollectionViewKind::Array,
        CollectionViewMutation::Growable,
    ));

    Engine::builder()
        .register_rust_type::<Vec<u8>>(binding)
        .build()
        .expect("Bytes values should support borrowed array views");
}

#[test]
fn type_binding_rejects_ambiguous_storage_and_receiver_capabilities() {
    let value_with_host_storage = Engine::builder()
        .register_rust_type::<ExternalValue>(external_value_binding(host_desc(101, 201)))
        .build();
    assert!(matches!(
        value_with_host_storage,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingStorage {
                name: "host::ExternalHost".to_owned(),
                storage: "value".to_owned(),
            }
    ));

    let exclusive_without_shared = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201)).receiver_capabilities(
                ReceiverCapabilities::OWNED.with(ReceiverCapability::Exclusive),
            ),
        )
        .build();
    assert!(matches!(
        exclusive_without_shared,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingCapabilities {
                name: "host::ExternalHost".to_owned(),
                bits: 5,
            }
    ));

    let owner = host_desc(101, 201).key;
    let method_exceeds_type = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201))
                .receiver_capabilities(ReceiverCapabilities::OWNED.with(ReceiverCapability::Shared))
                .method_desc(
                    NativeMethodDesc::new(owner, HostMethodId::new(302), "write")
                        .receiver(ReceiverCapability::Exclusive),
                ),
        )
        .build();
    assert!(matches!(
        method_exceeds_type,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingMethodReceiver {
                type_name: "host::ExternalHost".to_owned(),
                method: "write".to_owned(),
                receiver: "exclusive".to_owned(),
            }
    ));
}

#[test]
fn one_rust_type_cannot_map_to_two_interop_identities() {
    let result = Engine::builder()
        .register_rust_type::<ExternalValue>(external_value_binding(value_desc(102, "amount")))
        .register_rust_type::<ExternalValue>(external_value_binding(
            TypeDesc::new(TypeKey::new(TypeId::new(103), "host::OtherValue"))
                .kind(TypeKind::ScriptStruct),
        ))
        .build();
    assert!(matches!(
        result,
        Err(error)
            if error.kind == EngineErrorKind::DuplicateRustTypeBinding {
                first: 102,
                second: 103,
            }
    ));
}

#[test]
fn method_receiver_requirement_participates_in_type_abi() {
    let build = |receiver| {
        let owner = host_desc(101, 201).key;
        Engine::builder()
            .register_rust_type::<ExternalHost>(TypeBinding::host(host_desc(101, 201)).method_desc(
                NativeMethodDesc::new(owner, HostMethodId::new(302), "touch").receiver(receiver),
            ))
            .build()
            .expect("receiver-specific binding should seal")
    };
    let shared = build(ReceiverCapability::Shared).type_bindings();
    let exclusive = build(ReceiverCapability::Exclusive).type_bindings();

    assert_ne!(
        shared
            .get_for::<ExternalHost>()
            .expect("shared binding")
            .abi_fingerprint,
        exclusive
            .get_for::<ExternalHost>()
            .expect("exclusive binding")
            .abi_fingerprint
    );
    assert_ne!(shared.checksum(), exclusive.checksum());
}

#[test]
fn registered_value_codec_round_trips_through_script_execution() {
    let engine = Engine::builder()
        .register_rust_type::<ExternalValue>(external_value_binding(value_desc(102, "amount")))
        .build()
        .expect("value binding should seal");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn increase(value: host::ExternalValue) {
    return host::ExternalValue { amount: value.amount + 5 };
}
"#,
        )
        .expect("registered value type should compile in script signatures and literals");
    let codec = engine
        .type_bindings()
        .value_codec::<ExternalValue>()
        .expect("registered typed value codec");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let output = runtime
        .call(
            "increase",
            CallArgs::from_positional([codec.encode(ExternalValue { amount: 7 })]),
            CallOptions::unbounded(),
        )
        .expect("script should transform the registered value");
    let output = runtime
        .value_to_owned(&output)
        .expect("script output should materialize");

    assert_eq!(
        codec.decode(&output).expect("output should decode"),
        ExternalValue { amount: 12 }
    );
}

#[test]
fn registered_value_constructor_is_callable_from_vela_and_projected_as_type_fact() {
    let engine = Engine::builder()
        .register_rust_type::<ExternalValue>(
            external_value_binding(value_desc(102, "amount")).constructor_fn(
                external_value_constructor(TypeHint::i64()),
                construct_external_value,
            ),
        )
        .build()
        .expect("value constructor binding should seal");
    let type_bindings = engine.type_bindings();
    let binding = type_bindings
        .get_for::<ExternalValue>()
        .expect("value binding");
    assert!(binding.capabilities.contains(ReceiverCapability::Construct));
    assert_eq!(binding.constructor_ids, [FunctionId::new(401)]);
    assert_eq!(
        engine
            .registry()
            .type_binding_for_key(&value_desc(102, "amount").key)
            .expect("reflection binding")
            .constructor_ids,
        [FunctionId::new(401)]
    );
    let compiler_facts = RegistryFacts::from_compile_view(engine.compiler_registry())
        .expect("compiler binding facts");
    assert_eq!(
        compiler_facts
            .type_binding_fact("host::ExternalValue")
            .expect("compiler type binding")
            .constructor_ids,
        [FunctionId::new(401)]
    );

    let program = engine
        .compile_source_with_id(
            SourceId::new(3),
            r#"
fn make() {
    return host::ExternalValue::new(7);
}
"#,
        )
        .expect("qualified registered constructor should compile");
    let codec = engine
        .type_bindings()
        .value_codec::<ExternalValue>()
        .expect("registered value codec");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let output = runtime
        .call("make", CallArgs::new(), CallOptions::unbounded())
        .expect("registered constructor should execute");
    let output = runtime
        .value_to_owned(&output)
        .expect("constructor output should materialize");

    assert_eq!(
        codec.decode(&output).expect("constructor output codec"),
        ExternalValue { amount: 7 }
    );
}

#[test]
fn constructor_signature_participates_in_type_binding_abi() {
    let build = |hint| {
        Engine::builder()
            .register_rust_type::<ExternalValue>(
                external_value_binding(value_desc(102, "amount"))
                    .constructor_fn(external_value_constructor(hint), construct_external_value),
            )
            .build()
            .expect("constructor binding should seal")
    };
    let i64_binding = build(TypeHint::i64()).type_bindings();
    let i32_binding = build(TypeHint::i32()).type_bindings();

    assert_ne!(
        i64_binding
            .get_for::<ExternalValue>()
            .expect("i64 constructor binding")
            .abi_fingerprint,
        i32_binding
            .get_for::<ExternalValue>()
            .expect("i32 constructor binding")
            .abi_fingerprint
    );
    assert_ne!(i64_binding.checksum(), i32_binding.checksum());
}

#[test]
fn type_binding_rejects_constructor_with_wrong_owner_or_return_type() {
    let wrong_owner = Engine::builder()
        .register_rust_type::<ExternalValue>(
            external_value_binding(value_desc(102, "amount")).constructor_fn(
                crate::native::NativeFunctionDesc::new(
                    "host::OtherValue::new",
                    FunctionId::new(402),
                )
                .returns(TypeHint::Record(value_desc(102, "amount").key)),
                construct_external_value,
            ),
        )
        .build();
    assert!(matches!(
        wrong_owner,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingConstructor {
                type_name: "host::ExternalValue".to_owned(),
                constructor: "host::OtherValue::new".to_owned(),
                reason: "qualified name is not directly owned by the bound type",
            }
    ));

    let wrong_return = Engine::builder()
        .register_rust_type::<ExternalValue>(
            external_value_binding(value_desc(102, "amount")).constructor_fn(
                crate::native::NativeFunctionDesc::new(
                    "host::ExternalValue::new",
                    FunctionId::new(403),
                )
                .returns(TypeHint::i64()),
                construct_external_value,
            ),
        )
        .build();
    assert!(matches!(
        wrong_return,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingConstructor {
                type_name: "host::ExternalValue".to_owned(),
                constructor: "host::ExternalValue::new".to_owned(),
                reason: "declared return type is not the bound type",
            }
    ));

    let forged_host_ref = Engine::builder()
        .register_rust_type::<ExternalHost>(TypeBinding::host(host_desc(101, 201)).constructor_fn(
            external_host_constructor(),
            |_args, _host| {
                Ok(OwnedValue::HostRef(vela_host::path::HostRef::new(
                    HostTypeId::new(201),
                    vela_common::HostObjectId::new(1),
                    1,
                )))
            },
        ))
        .build();
    assert!(matches!(
        forged_host_ref,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingConstructor {
                type_name: "host::ExternalHost".to_owned(),
                constructor: "host::ExternalHost::new".to_owned(),
                reason: "a Host binding requires a host-owned factory",
            }
    ));
}

#[test]
fn host_constructor_retains_rust_object_outside_gc_across_runtime_calls() {
    let engine = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201))
                .host_constructor_fn(external_host_constructor(), construct_external_host),
        )
        .build()
        .expect("host factory binding should seal");
    let type_bindings = engine.type_bindings();
    let binding = type_bindings
        .get_for::<ExternalHost>()
        .expect("host binding");
    assert!(binding.capabilities.contains(ReceiverCapability::Construct));
    assert_eq!(binding.constructor_ids, [FunctionId::new(411)]);

    let program = engine
        .compile_source_with_id(
            SourceId::new(4),
            r#"
fn make(amount: i64) {
    return host::ExternalHost::new(amount);
}

fn add_five(value: host::ExternalHost) {
    value.value += 5;
    return value.value;
}
"#,
        )
        .expect("host constructor and field access should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let handle = runtime
        .call(
            "make",
            CallArgs::from_positional([OwnedValue::from(7_i64)]),
            CallOptions::unbounded(),
        )
        .expect("host factory should construct into the Runtime arena");
    let result = runtime
        .call(
            "add_five",
            CallArgs::new().with_vela_value(handle.clone()),
            CallOptions::unbounded(),
        )
        .expect("constructed host should remain live across calls");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::from(12_i64))
    );
    let result = runtime
        .call(
            "add_five",
            CallArgs::new().with_vela_value(handle),
            CallOptions::unbounded(),
        )
        .expect("the same host handle should preserve Rust-side mutations");
    assert_eq!(
        runtime.value_to_owned(&result),
        Ok(OwnedValue::from(17_i64))
    );
}

#[test]
fn exclusive_method_rejects_shared_view_and_accepts_mut_view() {
    let owner = host_desc(101, 201).key;
    let engine = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201)).native_method_fn(
                NativeMethodDesc::new(owner, HostMethodId::new(302), "touch")
                    .receiver(ReceiverCapability::Exclusive),
                |_, _, _| Ok(OwnedValue::Unit),
            ),
        )
        .build()
        .expect("exclusive method binding should seal");
    let program = engine
        .compile_source_with_id(
            SourceId::new(2),
            r#"
fn touch(value: host::ExternalHost) {
    value.touch();
    return true;
}
"#,
        )
        .expect("exclusive method should compile for dynamic call-bound receiver access");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");
    let mut host = ExternalHost { amount: 0 };

    let shared_error = runtime
        .call(
            "touch",
            CallArgs::new().with_host_ref("value", &host),
            CallOptions::unbounded(),
        )
        .expect_err("shared Rust view must not enter an exclusive method");
    assert!(matches!(
        shared_error.kind_ref(),
        VmErrorKind::Host(vela_host::error::HostErrorKind::PermissionDenied {
            action: "call exclusive receiver method",
            ..
        })
    ));

    let output = runtime
        .call(
            "touch",
            CallArgs::new().with_host_mut("value", &mut host),
            CallOptions::unbounded(),
        )
        .expect("mutable Rust view should enter an exclusive method");
    assert_eq!(runtime.value_to_owned(&output), Ok(OwnedValue::Bool(true)));
}
