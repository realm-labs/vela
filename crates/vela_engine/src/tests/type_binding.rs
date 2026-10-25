use vela_common::SourceId;
use vela_def::{FieldId, TypeId};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::args::FromScriptArg;
use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::runtime::{CallArgs, CallOptions, Runtime};
use crate::type_binding::{TypeBinding, ValueCodec};

#[derive(Debug, Eq, PartialEq)]
struct ExternalValue {
    amount: i64,
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
fn owned_type_binding_is_sealed_into_engine_and_reflection_registry() {
    let binding = external_value_binding(value_desc(102, "amount"));
    let proof = binding.interop_contract();
    let engine = Engine::builder()
        .install_type_binding(binding)
        .build()
        .expect("value binding");
    let bindings = engine.type_bindings();
    let registered = bindings.get_for::<ExternalValue>().expect("typed binding");
    assert_eq!(registered.id, proof.type_id);
    assert!(bindings.matches_contract(proof));
    assert_eq!(
        engine.registry().type_binding(proof.type_id),
        Some(registered)
    );
    assert!(
        !bindings.matches_contract(vela_common::InteropBindingContract::new(
            proof.type_id,
            vela_common::TypeAbiFingerprint::new(proof.abi_fingerprint.get().wrapping_add(1)),
        ))
    );
}

#[test]
fn type_binding_fingerprint_ignores_docs_but_tracks_structural_abi() {
    let first = Engine::builder()
        .install_type_binding::<ExternalValue>(external_value_binding(
            value_desc(102, "amount").docs("first docs"),
        ))
        .build()
        .expect("first binding");
    let second = Engine::builder()
        .install_type_binding::<ExternalValue>(external_value_binding(
            value_desc(102, "amount").docs("different docs"),
        ))
        .build()
        .expect("second binding");
    let changed = Engine::builder()
        .install_type_binding::<ExternalValue>(external_value_binding(value_desc(102, "total")))
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
fn one_rust_type_cannot_map_to_two_interop_identities() {
    let result = Engine::builder()
        .install_type_binding::<ExternalValue>(external_value_binding(value_desc(102, "amount")))
        .install_type_binding::<ExternalValue>(external_value_binding(
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
fn registered_value_codec_round_trips_through_script_execution() {
    let engine = Engine::builder()
        .install_type_binding::<ExternalValue>(external_value_binding(value_desc(102, "amount")))
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
