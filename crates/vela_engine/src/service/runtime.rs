//! Service-owned Runtime leases for Vela-selected service calls.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use vela_bytecode::{ExecutableGenerationId, LinkedArtifact};
use vela_common::{ServiceCallMode, ServiceId, ServiceMethodId};
use vela_vm::error::{VmError, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;
use crate::engine::Engine;
use crate::runtime::{Runtime, RuntimeBuildError};

/// Object-safe future returned by generated async service dispatch methods.
#[doc(hidden)]
pub type ServiceFuture<'call, T> = Pin<Box<dyn Future<Output = T> + Send + 'call>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceCallTarget {
    pub mode: ServiceCallMode,
    pub service: ServiceId,
    pub method: ServiceMethodId,
}

impl ServiceCallTarget {
    #[must_use]
    pub const fn new(mode: ServiceCallMode, service: ServiceId, method: ServiceMethodId) -> Self {
        Self {
            mode,
            service,
            method,
        }
    }
}

/// Immutable, generation-pinned routing for compiler-provided `base` and
/// `services` calls made from a Vela service method.
#[doc(hidden)]
pub trait ServiceCallDispatcher: Send + Sync {
    fn dispatch(
        &self,
        target: ServiceCallTarget,
        args: &[OwnedValue],
        context: &mut NativeCallContext<'_, '_>,
    ) -> VmResult<OwnedValue>;

    fn dispatch_async<'call, 'host, 'lease>(
        &'call self,
        target: ServiceCallTarget,
        args: &'call [OwnedValue],
        leases: &'call mut [vela_host::lease::ErasedHostLease<'lease>],
        context: &'call mut NativeCallContext<'_, 'host>,
    ) -> ServiceFuture<'call, VmResult<OwnedValue>>
    where
        'lease: 'call;
}

/// Shared Runtime authority owned by a generated service application.
///
/// Business context parameters are ordinary call-scoped Host arguments. They
/// never need to store Runtime state or implement a Runtime-authority trait.
#[derive(Clone)]
pub struct ServiceRuntimeBinding {
    inner: Arc<ServiceRuntimePool>,
}

struct ServiceRuntimePool {
    engine: Engine,
    cached: parking_lot::Mutex<Option<CachedRuntime>>,
}

impl ServiceRuntimeBinding {
    #[must_use]
    pub fn for_engine(engine: Engine) -> Self {
        Self {
            inner: Arc::new(ServiceRuntimePool {
                engine,
                cached: parking_lot::Mutex::new(None),
            }),
        }
    }

    #[must_use]
    pub fn cached_generation(&self) -> Option<ExecutableGenerationId> {
        self.inner
            .cached
            .lock()
            .as_ref()
            .map(|cached| cached.generation)
    }

    pub fn invoke(
        &self,
        artifact: &Arc<LinkedArtifact>,
        invoke: impl FnOnce(&mut Runtime) -> VmResult<OwnedValue>,
    ) -> Result<OwnedValue, ServiceInvocationError> {
        let mut lease = self.lease(artifact)?;
        invoke(lease.runtime()).map_err(ServiceInvocationError::Vm)
    }

    pub fn lease(
        &self,
        artifact: &Arc<LinkedArtifact>,
    ) -> Result<ServiceRuntimeLease, ServiceInvocationError> {
        ServiceRuntimeLease::new(self.clone(), artifact)
    }
}

/// Cancellation-safe owner for one Runtime temporarily removed from the
/// service-owned bounded cache.
#[doc(hidden)]
pub struct ServiceRuntimeLease {
    binding: ServiceRuntimeBinding,
    artifact: Arc<LinkedArtifact>,
    runtime: Option<Runtime>,
}

impl ServiceRuntimeLease {
    fn new(
        binding: ServiceRuntimeBinding,
        artifact: &Arc<LinkedArtifact>,
    ) -> Result<Self, ServiceInvocationError> {
        let cached = binding.inner.cached.lock().take();
        let runtime = match cached {
            Some(cached) if cached.generation == artifact.generation() => cached.runtime,
            Some(_) | None => {
                Runtime::from_linked_artifact(binding.inner.engine.clone(), Arc::clone(artifact))
                    .map_err(ServiceInvocationError::RuntimeBuild)?
            }
        };
        Ok(Self {
            binding,
            artifact: Arc::clone(artifact),
            runtime: Some(runtime),
        })
    }

    pub fn runtime(&mut self) -> &mut Runtime {
        self.runtime
            .as_mut()
            .expect("service Runtime lease is active")
    }
}

impl Drop for ServiceRuntimeLease {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let mut cached = self.binding.inner.cached.lock();
            if cached.is_none() {
                *cached = Some(CachedRuntime {
                    generation: self.artifact.generation(),
                    runtime,
                });
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServiceInvocationError {
    RuntimeBuild(RuntimeBuildError),
    Vm(VmError),
}

impl fmt::Display for ServiceInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeBuild(error) => write!(formatter, "service Runtime build failed: {error}"),
            Self::Vm(error) => write!(formatter, "Vela service invocation failed: {error}"),
        }
    }
}

impl std::error::Error for ServiceInvocationError {}

struct CachedRuntime {
    generation: ExecutableGenerationId,
    runtime: Runtime,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::ServiceRuntimeBinding;
    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions};
    use vela_vm::owned_value::OwnedValue;

    #[test]
    fn service_owned_binding_reuses_runtime_without_business_context_authority() {
        let engine = Engine::builder().build().expect("engine");
        let artifact = linked(&engine, "fn adjust(value) { return value + 1; }");
        let function = artifact
            .verified_mir()
            .roots()
            .next()
            .expect("one compiled function")
            .0;
        let binding = ServiceRuntimeBinding::for_engine(engine);

        let output = binding
            .invoke(&artifact, |runtime| {
                let output = runtime.call_stable_function(
                    function,
                    "adjust",
                    CallArgs::from_positional([OwnedValue::i64(5)]),
                    CallOptions::unbounded(),
                )?;
                runtime.value_to_owned(&output)
            })
            .expect("service-owned Runtime binding");
        assert_eq!(output, OwnedValue::i64(6));
        assert_eq!(binding.cached_generation(), Some(artifact.generation()));
    }

    fn linked(engine: &Engine, source: &str) -> Arc<vela_bytecode::LinkedArtifact> {
        let compiled = engine.compile_source(source).expect("compiled source");
        engine
            .link_compiled_program(compiled)
            .expect("linked source")
    }
}
