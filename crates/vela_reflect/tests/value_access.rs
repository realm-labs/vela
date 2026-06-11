use vela_common::{HostMethodId, HostObjectId, HostTypeId};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;
use vela_reflect::error::ReflectErrorKind;
use vela_reflect::registry::{FieldDesc, MethodDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_reflect::value::{ReflectContext, ReflectValue, call, get, set};

fn player_ref() -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
}

fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(FieldDesc::new(FieldId::new(1), "id"))
            .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
            .method(MethodDesc::new(HostMethodId::new(5), "grant_exp")),
    );
    registry
}

fn adapter_with_level(value: HostValue) -> MockStateAdapter {
    let mut adapter = MockStateAdapter::new();
    adapter.insert_diagnostic_path_value(HostPath::new(player_ref()).field(FieldId::new(2)), value);
    adapter
}

#[test]
fn reflect_set_host_ref_creates_patch() {
    let registry = registry();
    let mut adapter = adapter_with_level(HostValue::Scalar(vela_common::ScalarValue::I64(9)));
    let mut tx = HostAccess::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &mut adapter,
        access: &mut tx,
    };

    set(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "level",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
    )
    .expect("reflect set");
}

#[test]
fn reflect_get_host_ref_reads_write_through_state() {
    let registry = registry();
    let mut adapter = adapter_with_level(HostValue::Scalar(vela_common::ScalarValue::I64(9)));
    let mut tx = HostAccess::new();
    let plan = HostTargetPlan::new(player_ref().type_id).field(FieldId::new(2));
    tx.write(
        &mut adapter,
        HostTargetInstance::new(player_ref(), &plan, &[]),
        HostValue::Scalar(vela_common::ScalarValue::I64(12)),
        None,
    )
    .expect("set host target");
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &mut adapter,
        access: &mut tx,
    };

    let value = get(&mut ctx, &ReflectValue::HostRef(player_ref()), "level").expect("reflect get");

    assert_eq!(
        value,
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn reflect_set_read_only_host_field_fails_without_patch() {
    let registry = registry();
    let mut adapter = adapter_with_level(HostValue::Scalar(vela_common::ScalarValue::I64(9)));
    let mut tx = HostAccess::new();
    let mut ctx = ReflectContext {
        registry: &registry,
        adapter: &mut adapter,
        access: &mut tx,
    };

    let error = set(
        &mut ctx,
        &ReflectValue::HostRef(player_ref()),
        "id",
        ReflectValue::Host(HostValue::Scalar(vela_common::ScalarValue::I64(10))),
    )
    .expect_err("read-only set");

    assert_eq!(
        error.kind,
        ReflectErrorKind::FieldNotWritable {
            type_name: "Player".to_owned(),
            field: "id".to_owned(),
            source_span: None,
        }
    );
}

#[test]
fn reflect_call_host_ref_writes_through_and_updates_adapter() {
    let registry = registry();
    let mut adapter = adapter_with_level(HostValue::Scalar(vela_common::ScalarValue::I64(9)));
    adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
    let mut tx = HostAccess::new();
    {
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &mut adapter,
            access: &mut tx,
        };

        let value = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Host(HostValue::Scalar(
                vela_common::ScalarValue::I64(20),
            ))],
        )
        .expect("reflect call");

        assert_eq!(value, ReflectValue::Host(HostValue::Null));
    }
    let calls = adapter.method_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].target.root, player_ref());
    assert_eq!(
        calls[0].target.target,
        HostTargetPlan::new(player_ref().type_id)
    );
    assert_eq!(calls[0].method, HostMethodId::new(5));
    assert_eq!(
        calls[0].args,
        vec![HostValue::Scalar(vela_common::ScalarValue::I64(20))]
    );
}
