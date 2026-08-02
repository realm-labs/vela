use vela_common::{Capability, HostMethodId, ReceiverCapability, stable_id};
use vela_engine::engine::Engine;
use vela_engine::method::NativeMethodDesc;
use vela_engine::native::{EffectSet, FunctionAccess, TypeHint};
use vela_engine::registration::{__private::NominalHostMethodFamily, VelaBindings};
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_engine::schema::ScriptHostSchema;
use vela_macros::{ScriptHost, Value};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, ScriptHost)]
#[vela(path = "test::EventRecorder")]
struct EventRecorder {
    total: i64,
}

trait TestEvent {
    fn amount(self) -> i64;
}

#[derive(Debug, Value)]
#[vela(path = "test::Added")]
struct Added {
    amount: i64,
}

impl TestEvent for Added {
    fn amount(self) -> i64 {
        self.amount
    }
}

#[derive(Debug, Value)]
#[vela(path = "test::Multiplied")]
struct Multiplied {
    factor: i64,
}

impl TestEvent for Multiplied {
    fn amount(self) -> i64 {
        self.factor * 10
    }
}

#[derive(Debug, Value)]
#[vela(path = "test::Tick")]
struct Tick;

impl TestEvent for Tick {
    fn amount(self) -> i64 {
        1
    }
}

#[derive(Debug, Value)]
#[vela(path = "test::NestedAmount")]
struct NestedAmount {
    value: i64,
}

#[derive(Debug, Value)]
#[vela(path = "test::NestedEvent")]
struct NestedEvent {
    amount: NestedAmount,
}

impl TestEvent for NestedEvent {
    fn amount(self) -> i64 {
        self.amount.value
    }
}

impl EventRecorder {
    fn publish<E: TestEvent>(&mut self, event: E) -> i64 {
        self.total += event.amount();
        self.total
    }
}

#[test]
fn nominal_method_family_dispatches_to_rust_monomorphized_instances() {
    let method_id = HostMethodId::new(u128::from(stable_id(
        "host_method",
        "test::EventRecorder",
        "publish",
    )));
    let desc = NativeMethodDesc::new(
        EventRecorder::script_host_type_desc().key,
        method_id,
        "publish",
    )
    .param("event", TypeHint::Any)
    .returns(TypeHint::i64())
    .effects(EffectSet::host_write())
    .receiver(ReceiverCapability::Exclusive)
    .access(FunctionAccess::public());
    let mut family = NominalHostMethodFamily::<EventRecorder>::new(desc);
    family.register_instance::<Added, _, _>(EventRecorder::publish::<Added>);
    family.register_instance::<Multiplied, _, _>(EventRecorder::publish::<Multiplied>);
    family.register_instance::<Tick, _, _>(EventRecorder::publish::<Tick>);
    family.register_instance::<NestedEvent, _, _>(EventRecorder::publish::<NestedEvent>);

    let mut bindings = VelaBindings::new();
    bindings
        .register_type(EventRecorder::vela_type())
        .register_methods(family.into_registration());
    let engine = Engine::builder()
        .capability(Capability::HostWrite)
        .register_bindings(bindings)
        .build()
        .expect("method family should seal");
    let program = engine
        .compile_source(
            r#"
pub fn main(recorder: EventRecorder) -> i64 {
    recorder.publish(test::Added { amount: 2 });
    recorder.publish(test::Multiplied { factor: 3 });
    recorder.publish(test::Tick {});
    return recorder.publish(test::NestedEvent {
        amount: test::NestedAmount { value: 4 },
    });
}
"#,
        )
        .expect("one method should accept both registered nominal values");
    let mut runtime = Runtime::new(engine, program).expect("method family runtime");
    let mut recorder = EventRecorder { total: 0 };
    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_mut("recorder", &mut recorder),
            CallOptions::unbounded(),
        )
        .expect("method family invocation");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(37)));
    assert_eq!(recorder.total, 37);
}
