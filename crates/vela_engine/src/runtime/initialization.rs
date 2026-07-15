use vela_common::Span;
use vela_host::error::HostResult;
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_vm::error::VmError;

use super::{
    CallArgs, CallOptions, RuntimeCallExecution, RuntimeImageStorage, RuntimeImpl, handles,
};

const DEFAULT_INITIALIZER_EXECUTION_UNITS: u64 = 100_000;
const DEFAULT_INITIALIZER_MEMORY_BYTES: usize = 1024 * 1024;
const DEFAULT_INITIALIZER_CALL_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeInitializationLimits {
    pub execution_units: u64,
    pub memory_bytes: usize,
    pub call_depth: usize,
}

impl RuntimeInitializationLimits {
    #[must_use]
    pub const fn new(execution_units: u64, memory_bytes: usize, call_depth: usize) -> Self {
        Self {
            execution_units,
            memory_bytes,
            call_depth,
        }
    }

    pub(super) const fn call_options(self) -> CallOptions {
        CallOptions::new(self.execution_units, self.memory_bytes, self.call_depth)
            .with_managed_heap(true)
    }
}

impl Default for RuntimeInitializationLimits {
    fn default() -> Self {
        Self::new(
            DEFAULT_INITIALIZER_EXECUTION_UNITS,
            DEFAULT_INITIALIZER_MEMORY_BYTES,
            DEFAULT_INITIALIZER_CALL_DEPTH,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeBuildError {
    Link(vela_bytecode::linker::LinkError),
    Initializer {
        state: String,
        source_span: Option<Span>,
        error: VmError,
    },
    MissingExternState {
        state: String,
        source_span: Option<Span>,
    },
}

impl std::fmt::Display for RuntimeBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Link(error) => write!(formatter, "runtime image link failed: {error:?}"),
            Self::Initializer { state, error, .. } => {
                write!(formatter, "state initializer for `{state}` failed: {error}")
            }
            Self::MissingExternState { state, .. } => {
                write!(formatter, "extern state `{state}` has no host binding")
            }
        }
    }
}

impl std::error::Error for RuntimeBuildError {}

impl From<vela_bytecode::linker::LinkError> for RuntimeBuildError {
    fn from(error: vela_bytecode::linker::LinkError) -> Self {
        Self::Link(error)
    }
}

pub struct RuntimeBuilder<I = super::OwnedImage>
where
    I: RuntimeImageStorage,
{
    runtime: RuntimeImpl<I>,
    limits: RuntimeInitializationLimits,
}

impl<I> RuntimeBuilder<I>
where
    I: RuntimeImageStorage,
{
    pub(super) fn new(runtime: RuntimeImpl<I>) -> Self {
        Self {
            runtime,
            limits: RuntimeInitializationLimits::default(),
        }
    }

    pub fn bind_extern_state<T>(&mut self, name: impl Into<String>, value: T) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.runtime.state.extern_states.bind_host(name, value)
    }

    #[must_use]
    pub fn with_initialization_limits(mut self, limits: RuntimeInitializationLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn build(mut self) -> Result<RuntimeImpl<I>, RuntimeBuildError> {
        if let Some((state, source_span)) = self
            .runtime
            .state
            .extern_states
            .missing_bindings(self.runtime.image.states())
            .into_iter()
            .next()
        {
            return Err(RuntimeBuildError::MissingExternState { state, source_span });
        }
        self.runtime.initialize_vm_states(self.limits)?;
        Ok(self.runtime)
    }
}

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub(super) fn initialize_vm_states(
        &mut self,
        limits: RuntimeInitializationLimits,
    ) -> Result<(), RuntimeBuildError> {
        let mut states = self
            .image
            .linked_program()
            .states()
            .iter()
            .filter(|state| state.storage == vela_bytecode::StateStorage::Vm)
            .cloned()
            .collect::<Vec<_>>();
        states.sort_by_key(|state| state.id);

        for state in states {
            let initializer = state
                .initializer
                .expect("verified VM state descriptor has an initializer");
            let code = self
                .image
                .linked_program()
                .function(initializer)
                .expect("verified state initializer handle resolves");
            let target = handles::EntryRequest {
                name: state.qualified_name.clone(),
                asyncness: code.asyncness,
                function: initializer,
                params: Vec::new(),
                param_defaults: Vec::new(),
                receiver: None,
            };
            let runtime_state = &mut self.state;
            let result = Self::call_runtime_args(RuntimeCallExecution {
                runtime_id: runtime_state.id,
                engine: self.image.engine(),
                registry_image: self.image.program_image(),
                artifact: self.image.linked_artifact(),
                hot_reload: self.hot_reload.as_ref(),
                extern_states: &mut runtime_state.extern_states,
                vm_states: &mut runtime_state.vm_states,
                sidecars: &mut runtime_state.sidecars,
                target,
                args: CallArgs::new(),
                budget: limits.call_options().budget(),
            })
            .map_err(|error| RuntimeBuildError::Initializer {
                state: state.qualified_name.clone(),
                source_span: error.source_span.or(state.source_span),
                error,
            })?;
            runtime_state
                .vm_states
                .values
                .insert(state.id, result.value());
        }
        Ok(())
    }
}
