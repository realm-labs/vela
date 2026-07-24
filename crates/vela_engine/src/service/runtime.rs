//! Explicit runtime authority for Vela-selected service calls.

use std::sync::Arc;

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};

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

    use super::{ServiceRuntimeAuthority, ServiceRuntimeSlot};
    use crate::engine::Engine;

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

    fn linked(engine: &Engine, source: &str) -> Arc<vela_bytecode::LinkedArtifact> {
        let compiled = engine.compile_source(source).expect("compiled source");
        engine
            .link_compiled_program(compiled)
            .expect("linked source")
    }
}
