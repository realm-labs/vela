use vela_common::{Capability, HostMethodId, ReceiverCapability, stable_id};
use vela_engine::engine::Engine;
use vela_engine::method::NativeMethodDesc;
use vela_engine::method_family::NominalHostMethodFamily;
use vela_engine::native::{EffectSet, FunctionAccess, TypeHint};
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

    let engine = family
        .install(
            Engine::builder()
                .capability(Capability::HostWrite)
                .register_type::<EventRecorder>(),
        )
        .build()
        .expect("method family should seal");
    let program = engine
        .compile_source(
            r#"
pub fn main(recorder: EventRecorder) -> i64 {
    recorder.publish(test::Added { amount: 2 });
    return recorder.publish(test::Multiplied { factor: 3 });
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

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(32)));
    assert_eq!(recorder.total, 32);
}
