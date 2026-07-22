use vela_macros::{ScriptHost, ScriptReflect, methods};
use vela_vm::error::VmResult;

use super::{DispatchAuthority, DispatchController, DispatchInvocation, DispatchRoot};
use crate::engine::Engine;
use crate::runtime::{RuntimeImage, SharedRuntime};

pub trait Handler<Message> {
    fn handle(&self, actor: &mut HostActor, message: Message) -> VmResult<i64>;
}

pub trait Service {
    fn quote(&self, actor: &mut HostActor, value: i64) -> VmResult<i64>;
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::example::Context")]
pub struct HostContext {
    #[script(get, set)]
    calls: i64,
}

pub struct HostTurn {
    root: DispatchRoot,
    runtime: SharedRuntime,
}

pub struct HostActor {
    turn: HostTurn,
    context: HostContext,
}

#[methods(path = "host::example::Context")]
impl HostContext {
    pub fn calls(&self) -> i64 {
        self.calls
    }
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::example::Worker")]
pub struct Worker {
    #[script(get)]
    bonus: i64,
}

#[derive(ScriptHost, ScriptReflect)]
#[script(path = "host::example::PricingService")]
pub struct PricingService {
    #[script(get)]
    offset: i64,
}

macro_rules! host_framework {
    (
        handler $handler:ident for $message:ident |
            $handler_self:ident, $handler_context:ident, $message_value:ident
        | $handler_body:block;
        service $service:ident |
            $service_self:ident, $service_context:ident, $service_value:ident
        | $service_body:block;
    ) => {
        #[vela_macros::methods(path = "host::example::Handler")]
        impl $handler {
            #[vela_macros::replaceable(
                path = "host::example::Handler::handle",
                authority = "turn",
                index = 0
            )]
            pub fn handle(
                &self,
                turn: &mut HostTurn,
                context: &mut HostContext,
                message: $message,
            ) -> VmResult<i64> {
                let _ = turn;
                let $handler_self = self;
                let $handler_context = context;
                let $message_value = message;
                $handler_body
            }
        }

        impl Handler<$message> for $handler {
            fn handle(&self, actor: &mut HostActor, message: $message) -> VmResult<i64> {
                let HostActor { turn, context } = actor;
                <$handler>::handle(self, turn, context, message)
            }
        }

        impl DispatchAuthority for HostTurn {
            fn vela_dispatch_root(&self) -> &DispatchRoot {
                &self.root
            }

            fn vela_dispatch_invocation(&mut self) -> VmResult<DispatchInvocation<'_>> {
                let Self { root, runtime, .. } = self;
                root.invocation(runtime)
            }
        }

        #[vela_macros::methods(path = "host::example::PricingService")]
        impl $service {
            #[vela_macros::replaceable(
                path = "host::example::PricingService::quote",
                authority = "turn",
                index = 1
            )]
            pub fn quote(
                &self,
                turn: &mut HostTurn,
                context: &mut HostContext,
                value: i64,
            ) -> VmResult<i64> {
                let _ = turn;
                let $service_self = self;
                let $service_context = context;
                let $service_value = value;
                $service_body
            }

            pub fn adjacent(&self, value: i64) -> i64 {
                value + self.offset
            }
        }

        impl Service for $service {
            fn quote(&self, actor: &mut HostActor, value: i64) -> VmResult<i64> {
                let HostActor { turn, context } = actor;
                <$service>::quote(self, turn, context, value)
            }
        }

        fn host_replaceable_slots() -> Vec<crate::dispatch::ReplaceableSlotDescriptor> {
            let mut slots = <$handler>::vela_replaceable_slots();
            slots.extend(<$service>::vela_replaceable_slots());
            slots
        }

        fn register_host_framework(
            builder: crate::builder::EngineBuilder,
        ) -> crate::builder::EngineBuilder {
            builder
                .register_host_type::<HostContext>()
                .register_host_type::<$handler>()
                .register_host_type::<$service>()
                .register_exports(HostContext::vela_inherent_exports())
                .register_exports(<$handler>::vela_inherent_exports())
                .register_exports(<$service>::vela_inherent_exports())
                .register_replaceable_slots(host_replaceable_slots())
        }
    };
}

host_framework! {
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
    let slots = host_replaceable_slots();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].index.get(), 0);
    assert_eq!(slots[1].index.get(), 1);
    let engine = register_host_framework(Engine::builder())
        .capability(vela_common::Capability::HostRead)
        .capability(vela_common::Capability::HostWrite)
        .build()
        .expect("engine");
    let program = engine
        .compile_source(
            r#"
#[override(host::example::Handler::handle)]
fn handle(worker: Worker, context: Context, message: i64) -> i64 {
context.calls += 1;
return message + worker.bonus + 1;
}

#[override(host::example::PricingService::quote)]
fn quote(service: PricingService, context: Context, value: i64) -> i64 {
context.calls += 1;
return service.adjacent(value) + 1;
}
"#,
        )
        .expect("host override program");
    let runtime = shared_runtime(engine, program);
    let controller = DispatchController::new(slots).expect("controller");
    let worker = Worker { bonus: 1 };
    let service = PricingService { offset: 1 };

    let mut fallback = HostActor {
        turn: HostTurn {
            root: DispatchRoot::pin(&controller),
            runtime: SharedRuntime::from_shared_image(runtime.shared_image())
                .expect("actor runtime"),
        },
        context: HostContext { calls: 0 },
    };
    assert_eq!(
        <Worker as Handler<i64>>::handle(&worker, &mut fallback, 40),
        Ok(41)
    );
    assert_eq!(
        <PricingService as Service>::quote(&service, &mut fallback, 40),
        Ok(41)
    );
    assert_eq!(fallback.context.calls(), 20);

    let candidate = controller.stage_current(&runtime).expect("override stage");
    controller.activate(candidate).expect("activate candidate");
    let mut active = HostActor {
        turn: HostTurn {
            root: DispatchRoot::pin(&controller),
            runtime: SharedRuntime::from_shared_image(runtime.shared_image())
                .expect("actor runtime"),
        },
        context: HostContext { calls: 0 },
    };
    assert_eq!(
        <Worker as Handler<i64>>::handle(&worker, &mut active, 40),
        Ok(42)
    );
    assert_eq!(
        <PricingService as Service>::quote(&service, &mut active, 40),
        Ok(42)
    );
    assert_eq!(active.context.calls(), 2);
    assert_eq!(service.adjacent(40), 41);
}

fn shared_runtime(
    engine: Engine,
    program: vela_bytecode::compiler::CompiledProgram,
) -> SharedRuntime {
    let image = RuntimeImage::new_compiled(engine, program).into_shared();
    SharedRuntime::from_shared_image(image).expect("staging runtime")
}
