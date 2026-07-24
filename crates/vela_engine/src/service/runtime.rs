//! Explicit runtime authority for Vela-selected service calls.

use std::any::{Any, TypeId};
use std::fmt;
use std::sync::Arc;

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};
use vela_vm::error::{VmError, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::runtime::{Runtime, RuntimeBuildError};

/// A service-set context that can lend one mutable Runtime to a selected Vela
/// method without placing mutable runtime state in an immutable generation.
///
/// Implementations commonly delegate to a [`ServiceRuntimeSlot`] stored behind
/// a mutex or another request-local owner. The Runtime must be absent from that
/// owner while `invoke` receives both it and `&mut Self`; this prevents aliasing
/// the Runtime through the host context passed into Vela.
pub trait ServiceRuntimeAuthority: Sized {
    fn take_service_runtime(
        &mut self,
        artifact: &Arc<LinkedArtifact>,
    ) -> Result<Runtime, RuntimeBuildError>;

    fn restore_service_runtime(&mut self, artifact: &Arc<LinkedArtifact>, runtime: Runtime);

    fn with_service_runtime<R>(
        &mut self,
        artifact: &Arc<LinkedArtifact>,
        invoke: impl FnOnce(&mut Runtime, &mut Self) -> R,
    ) -> Result<R, RuntimeBuildError> {
        let mut runtime = self.take_service_runtime(artifact)?;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| invoke(&mut runtime, self)))
        {
            Ok(output) => {
                self.restore_service_runtime(artifact, runtime);
                Ok(output)
            }
            Err(payload) => {
                self.restore_service_runtime(artifact, runtime);
                std::panic::resume_unwind(payload);
            }
        }
    }
}

type ErasedServiceInvocation<'call> =
    Box<dyn FnOnce(&mut Runtime, &mut dyn Any) -> VmResult<OwnedValue> + 'call>;

type ErasedRuntimeDispatch = for<'call> fn(
    &mut dyn Any,
    &Arc<LinkedArtifact>,
    ErasedServiceInvocation<'call>,
) -> Result<OwnedValue, ServiceInvocationError>;

/// Type-checked bridge from a generated service adapter to the concrete
/// service-set context that owns Runtime authority.
#[derive(Clone, Copy)]
pub struct ServiceRuntimeBinding {
    context_type: TypeId,
    context_name: &'static str,
    dispatch: ErasedRuntimeDispatch,
}

impl ServiceRuntimeBinding {
    #[must_use]
    pub fn for_context<C>() -> Self
    where
        C: ServiceRuntimeAuthority + 'static,
    {
        Self {
            context_type: TypeId::of::<C>(),
            context_name: std::any::type_name::<C>(),
            dispatch: dispatch_with_context::<C>,
        }
    }

    #[must_use]
    pub const fn context_name(self) -> &'static str {
        self.context_name
    }

    #[must_use]
    pub fn matches<T: 'static>(self) -> bool {
        self.context_type == TypeId::of::<T>()
    }

    pub fn invoke<T>(
        self,
        context: &mut T,
        artifact: &Arc<LinkedArtifact>,
        invoke: impl FnOnce(&mut Runtime, &mut T) -> VmResult<OwnedValue>,
    ) -> Result<OwnedValue, ServiceInvocationError>
    where
        T: 'static,
    {
        if !self.matches::<T>() {
            return Err(ServiceInvocationError::ContextTypeMismatch {
                expected: self.context_name,
                actual: std::any::type_name::<T>(),
            });
        }
        (self.dispatch)(
            context,
            artifact,
            Box::new(move |runtime, erased| {
                let typed = erased
                    .downcast_mut::<T>()
                    .expect("validated service context type must downcast");
                invoke(runtime, typed)
            }),
        )
    }
}

fn dispatch_with_context<C>(
    erased: &mut dyn Any,
    artifact: &Arc<LinkedArtifact>,
    invoke: ErasedServiceInvocation<'_>,
) -> Result<OwnedValue, ServiceInvocationError>
where
    C: ServiceRuntimeAuthority + 'static,
{
    let context =
        erased
            .downcast_mut::<C>()
            .ok_or(ServiceInvocationError::ContextTypeMismatch {
                expected: std::any::type_name::<C>(),
                actual: "erased service context",
            })?;
    context
        .with_service_runtime(artifact, |runtime, context| invoke(runtime, context))
        .map_err(ServiceInvocationError::RuntimeBuild)?
        .map_err(ServiceInvocationError::Vm)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServiceInvocationError {
    ContextTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    MissingRuntimeContext {
        service: String,
        method: String,
        expected: &'static str,
    },
    RuntimeBuild(RuntimeBuildError),
    Vm(VmError),
}

impl fmt::Display for ServiceInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextTypeMismatch { expected, actual } => write!(
                formatter,
                "service Runtime context expects `{expected}`, found `{actual}`"
            ),
            Self::MissingRuntimeContext {
                service,
                method,
                expected,
            } => write!(
                formatter,
                "Vela service method `{service}::{method}` requires mutable context `{expected}`"
            ),
            Self::RuntimeBuild(error) => write!(formatter, "service Runtime build failed: {error}"),
            Self::Vm(error) => write!(formatter, "Vela service invocation failed: {error}"),
        }
    }
}

impl std::error::Error for ServiceInvocationError {}

/// Request-local owner for the Runtime used by Vela-selected service methods.
///
/// One slot reuses a Runtime while calls target the same linked artifact and
/// replaces it when a newly published service generation carries a different
/// artifact. It deliberately does not belong to the immutable service
/// generation.
pub struct ServiceRuntimeSlot {
    engine: Engine,
    cached: Option<CachedRuntime>,
}

struct CachedRuntime {
    generation: ExecutableGenerationId,
    runtime: Runtime,
}

impl ServiceRuntimeSlot {
    #[must_use]
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            cached: None,
        }
    }

    pub fn take(&mut self, artifact: &Arc<LinkedArtifact>) -> Result<Runtime, RuntimeBuildError> {
        if self
            .cached
            .as_ref()
            .is_some_and(|cached| cached.generation == artifact.generation())
        {
            return Ok(self
                .cached
                .take()
                .expect("matching cache was present")
                .runtime);
        }
        self.cached = None;
        Runtime::from_linked_artifact(self.engine.clone(), Arc::clone(artifact))
    }

    pub fn restore(&mut self, artifact: &Arc<LinkedArtifact>, runtime: Runtime) {
        self.cached = Some(CachedRuntime {
            generation: artifact.generation(),
            runtime,
        });
    }

    #[must_use]
    pub fn cached_generation(&self) -> Option<ExecutableGenerationId> {
        self.cached.as_ref().map(|cached| cached.generation)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        ServiceInvocationError, ServiceRuntimeAuthority, ServiceRuntimeBinding, ServiceRuntimeSlot,
    };
    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions};
    use vela_vm::owned_value::OwnedValue;

    struct RequestContext {
        slot: ServiceRuntimeSlot,
        calls: usize,
    }

    impl ServiceRuntimeAuthority for RequestContext {
        fn take_service_runtime(
            &mut self,
            artifact: &Arc<vela_bytecode::LinkedArtifact>,
        ) -> Result<crate::runtime::Runtime, crate::runtime::RuntimeBuildError> {
            self.slot.take(artifact)
        }

        fn restore_service_runtime(
            &mut self,
            artifact: &Arc<vela_bytecode::LinkedArtifact>,
            runtime: crate::runtime::Runtime,
        ) {
            self.slot.restore(artifact, runtime);
        }
    }

    #[test]
    fn context_lends_and_restores_the_exact_artifact_runtime() {
        let engine = Engine::builder().build().expect("engine");
        let first = linked(&engine, "fn first(value) { return value + 1; }");
        let second = linked(&engine, "fn second(value) { return value + 2; }");
        let mut context = RequestContext {
            slot: ServiceRuntimeSlot::new(engine),
            calls: 0,
        };

        context
            .with_service_runtime(&first, |_runtime, context| {
                assert_eq!(context.slot.cached_generation(), None);
                context.calls += 1;
            })
            .expect("first runtime");
        assert_eq!(context.slot.cached_generation(), Some(first.generation()));

        context
            .with_service_runtime(&first, |_runtime, context| {
                assert_eq!(context.slot.cached_generation(), None);
                context.calls += 1;
            })
            .expect("reused runtime");
        assert_eq!(context.slot.cached_generation(), Some(first.generation()));

        context
            .with_service_runtime(&second, |_runtime, context| {
                assert_eq!(context.slot.cached_generation(), None);
                context.calls += 1;
            })
            .expect("replacement runtime");
        assert_eq!(context.slot.cached_generation(), Some(second.generation()));
        assert_eq!(context.calls, 3);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), _> =
                context.with_service_runtime(&second, |_runtime, _context| panic!("service panic"));
        }));
        assert!(panic.is_err());
        assert_eq!(context.slot.cached_generation(), Some(second.generation()));
    }

    #[test]
    fn erased_binding_enters_only_its_declared_context_type() {
        let engine = Engine::builder().build().expect("engine");
        let artifact = linked(&engine, "fn adjust(value) { return value + 1; }");
        let function = artifact
            .verified_mir()
            .roots()
            .next()
            .expect("one compiled function")
            .0;
        let mut context = RequestContext {
            slot: ServiceRuntimeSlot::new(engine),
            calls: 0,
        };
        let binding = ServiceRuntimeBinding::for_context::<RequestContext>();

        let output = binding
            .invoke(&mut context, &artifact, |runtime, context| {
                context.calls += 1;
                let output = runtime.call_stable_function(
                    function,
                    "adjust",
                    CallArgs::from_positional([OwnedValue::i64(5)]),
                    CallOptions::unbounded(),
                )?;
                runtime.value_to_owned(&output)
            })
            .expect("typed service binding");
        assert_eq!(output, OwnedValue::i64(6));
        assert_eq!(context.calls, 1);

        let mut wrong = 0_u32;
        assert!(matches!(
            binding
                .invoke(&mut wrong, &artifact, |_runtime, _wrong| {
                    Ok(OwnedValue::Unit)
                })
                .expect_err("wrong context type"),
            ServiceInvocationError::ContextTypeMismatch { .. }
        ));
    }

    fn linked(engine: &Engine, source: &str) -> Arc<vela_bytecode::LinkedArtifact> {
        let compiled = engine.compile_source(source).expect("compiled source");
        engine
            .link_compiled_program(compiled)
            .expect("linked source")
    }
}
