use std::sync::Arc;

use parking_lot::Mutex;
use vela_macros::{ScriptHost, ScriptReflect, methods};
use vela_vm::error::VmResult;

use super::{DispatchAuthority, DispatchController, DispatchRoot, SharedDispatchRuntime};
use crate::engine::Engine;
use crate::runtime::{RuntimeImage, SharedRuntime};

pub trait Handler<Message> {
    fn handle(&self, context: &mut P9Context, message: Message) -> VmResult<i64>;
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::p9::Context")]
pub struct P9Context {
    #[script(get, set)]
    calls: i64,
    #[script(skip)]
    root: DispatchRoot,
}

#[methods(path = "host::p9::Context")]
impl P9Context {
    pub fn calls(&self) -> i64 {
        self.calls
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::p9::Worker")]
pub struct Worker {
    #[script(get)]
    bonus: i64,
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::p9::PricingService")]
pub struct PricingService {
    #[script(get)]
    offset: i64,
}

macro_rules! p9_lattice {
    (
        handler $handler:ident for $message:ident |
            $handler_self:ident, $handler_context:ident, $message_value:ident
        | $handler_body:block;
        service $service:ident |
            $service_self:ident, $service_context:ident, $service_value:ident
        | $service_body:block;
    ) => {
        #[vela_macros::methods(path = "host::p9::Handler")]
        impl $handler {
            #[vela_macros::replaceable(
                path = "host::p9::Handler::handle",
                authority = "context",
                index = 0
            )]
            pub fn handle(&self, context: &mut P9Context, message: $message) -> VmResult<i64> {
                let $handler_self = self;
                let $handler_context = context;
                let $message_value = message;
                $handler_body
            }
        }

        impl Handler<$message> for $handler {
            fn handle(&self, context: &mut P9Context, message: $message) -> VmResult<i64> {
                <$handler>::handle(self, context, message)
            }
        }

        impl DispatchAuthority for P9Context {
            fn vela_dispatch_root(&self) -> &DispatchRoot {
                &self.root
            }
        }

        #[vela_macros::methods(path = "host::p9::PricingService")]
        impl $service {
            #[vela_macros::replaceable(
                path = "host::p9::PricingService::quote",
                authority = "context",
                index = 1
            )]
            pub fn quote(&self, context: &mut P9Context, value: i64) -> VmResult<i64> {
                let $service_self = self;
                let $service_context = context;
                let $service_value = value;
                $service_body
            }

            pub fn adjacent(&self, value: i64) -> i64 {
                value + self.offset
            }
        }

        fn p9_replaceable_slots() -> Vec<crate::dispatch::ReplaceableSlotDescriptor> {
            let mut slots = <$handler>::vela_replaceable_slots();
            slots.extend(<$service>::vela_replaceable_slots());
            slots
        }

        fn register_p9_lattice(
            builder: crate::builder::EngineBuilder,
        ) -> crate::builder::EngineBuilder {
            builder
                .register_host_type::<P9Context>()
                .register_host_type::<$handler>()
                .register_host_type::<$service>()
                .register_exports(P9Context::vela_inherent_exports())
                .register_exports(<$handler>::vela_inherent_exports())
                .register_exports(<$service>::vela_inherent_exports())
                .register_replaceable_slots(p9_replaceable_slots())
        }
    };
}

p9_lattice! {
    handler Worker for i64 |worker, context, message| {
        context.calls += 10;
        Ok(message + worker.bonus)
    };
    service PricingService |service, context, value| {
        context.calls += 10;
        Ok(service.adjacent(value))
    };
}

#[test]
fn host_business_macro_hides_slots_authority_and_handler_proxy_plumbing() {
    let slots = p9_replaceable_slots();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].index.get(), 0);
    assert_eq!(slots[1].index.get(), 1);
    let engine = register_p9_lattice(Engine::builder())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::p9::Handler::handle)]
fn handle(worker: Worker, context: Context, message: i64) -> i64 {
context.calls += 1;
return message + worker.bonus + 1;
}

#[override(host::p9::PricingService::quote)]
fn quote(service: PricingService, context: Context, value: i64) -> i64 {
context.calls += 1;
return service.adjacent(value) + 1;
}
"#,
        )
        .expect("p9 override program");
    let runtime = shared_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let worker = Worker { bonus: 1 };
    let service = PricingService { offset: 1 };

    let mut fallback = P9Context {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("fallback root"),
    };
    assert_eq!(
        <Worker as Handler<i64>>::handle(&worker, &mut fallback, 40),
        Ok(41)
    );
    assert_eq!(service.quote(&mut fallback, 40), Ok(41));
    assert_eq!(fallback.calls(), 20);

    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut active = P9Context {
        calls: 0,
        root: DispatchRoot::pin(&controller, Arc::clone(&runtime)).expect("active root"),
    };
    assert_eq!(
        <Worker as Handler<i64>>::handle(&worker, &mut active, 40),
        Ok(42)
    );
    assert_eq!(service.quote(&mut active, 40), Ok(42));
    assert_eq!(active.calls(), 2);
    assert_eq!(service.adjacent(40), 41);
}

fn shared_runtime(
    engine: Engine,
    program: vela_bytecode::compiler::CompiledProgram,
) -> SharedDispatchRuntime {
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    Arc::new(Mutex::new(
        SharedRuntime::from_shared_image(image).expect("shared runtime"),
    ))
}
