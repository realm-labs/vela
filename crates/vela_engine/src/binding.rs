//! Runtime authority for compiler-schema-backed generated Rust bindings.

use std::fmt;

use vela_bytecode::{RustBindingCallableIdentity, RustBindingSchema};
use vela_common::{Capability, CapabilitySet, SourceId, Span};
use vela_def::{FunctionId, MethodId, TypeId};

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::context::NativeCallContext;
use crate::runtime::handles::StableVelaFunction;
use crate::runtime::{CallArgs, CallOptions, Runtime};

pub use vela_vm::error::{VmError, VmErrorKind, VmResult};
pub use vela_vm::owned_value::OwnedValue;

const DEFAULT_BINDING_EXECUTION_UNITS: u64 = 1_000_000;
const DEFAULT_BINDING_MEMORY_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_BINDING_CALL_DEPTH: usize = 128;

pub type BindingResult<T> = Result<T, BindingError>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingCallableIdentitySpec {
    Function(u128),
    Method { owner: u128, method: u128 },
}

impl BindingCallableIdentitySpec {
    const fn runtime(self) -> RustBindingCallableIdentity {
        match self {
            Self::Function(function) => {
                RustBindingCallableIdentity::Function(FunctionId::new(function))
            }
            Self::Method { owner, method } => RustBindingCallableIdentity::Method {
                owner: TypeId::new(owner),
                method: MethodId::new(method),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingCallableSpec {
    pub public_path: &'static str,
    pub identity: BindingCallableIdentitySpec,
    pub executable: u128,
    pub contract_fingerprint: u64,
    pub effect_bits: u32,
    pub source: Span,
}

impl BindingCallableSpec {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn function(
        public_path: &'static str,
        identity: u128,
        executable: u128,
        contract_fingerprint: u64,
        effect_bits: u32,
        source: u32,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            public_path,
            identity: BindingCallableIdentitySpec::Function(identity),
            executable,
            contract_fingerprint,
            effect_bits,
            source: Span::new(SourceId::new(source), start, end),
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn method(
        public_path: &'static str,
        owner: u128,
        method: u128,
        executable: u128,
        contract_fingerprint: u64,
        effect_bits: u32,
        source: u32,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            public_path,
            identity: BindingCallableIdentitySpec::Method { owner, method },
            executable,
            contract_fingerprint,
            effect_bits,
            source: Span::new(SourceId::new(source), start, end),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingSchemaSpec {
    pub version: u32,
    pub checksum: u64,
    pub types: &'static [BindingTypeSpec],
    pub callables: &'static [BindingCallableSpec],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingTypeSpec {
    pub public_path: &'static str,
    pub type_id: u128,
    pub schema_fingerprint: u64,
    pub source: Span,
}

impl BindingTypeSpec {
    #[must_use]
    pub const fn new(
        public_path: &'static str,
        type_id: u128,
        schema_fingerprint: u64,
        source: u32,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            public_path,
            type_id,
            schema_fingerprint,
            source: Span::new(SourceId::new(source), start, end),
        }
    }
}

impl BindingSchemaSpec {
    #[must_use]
    pub const fn new(
        version: u32,
        checksum: u64,
        callables: &'static [BindingCallableSpec],
    ) -> Self {
        Self {
            version,
            checksum,
            types: &[],
            callables,
        }
    }

    #[must_use]
    pub const fn with_types(mut self, types: &'static [BindingTypeSpec]) -> Self {
        self.types = types;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BindingCallable {
    schema: &'static BindingSchemaSpec,
    index: usize,
}

impl BindingCallable {
    #[must_use]
    pub const fn new(schema: &'static BindingSchemaSpec, index: usize) -> Self {
        Self { schema, index }
    }

    fn spec(self) -> VmResult<&'static BindingCallableSpec> {
        self.schema.callables.get(self.index).ok_or_else(|| {
            vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeMismatch {
                operation: "generated binding callable index",
            })
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingError {
    pub kind: BindingErrorKind,
    pub public_path: Option<&'static str>,
    pub source: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingErrorKind {
    SchemaVersion {
        expected: u32,
        actual: u32,
    },
    MissingCallable,
    IncompatibleCallable {
        expected_fingerprint: u64,
        actual_fingerprint: u64,
    },
    MissingType,
    IncompatibleType {
        expected_fingerprint: u64,
        actual_fingerprint: u64,
    },
    ReentryUnavailable,
}

impl BindingError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self.kind {
            BindingErrorKind::SchemaVersion { .. } => "binding.schema.version",
            BindingErrorKind::MissingCallable => "binding.callable.missing",
            BindingErrorKind::IncompatibleCallable { .. } => "binding.callable.incompatible",
            BindingErrorKind::MissingType => "binding.type.missing",
            BindingErrorKind::IncompatibleType { .. } => "binding.type.incompatible",
            BindingErrorKind::ReentryUnavailable => "binding.reentry.unavailable",
        }
    }
}

impl fmt::Display for BindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            BindingErrorKind::SchemaVersion { expected, actual } => write!(
                formatter,
                "generated binding schema version {expected} does not match active version {actual}"
            ),
            BindingErrorKind::MissingCallable => write!(
                formatter,
                "generated binding target `{}` is missing from the active artifact",
                self.public_path.unwrap_or("<unknown>")
            ),
            BindingErrorKind::IncompatibleCallable {
                expected_fingerprint,
                actual_fingerprint,
            } => write!(
                formatter,
                "generated binding target `{}` has contract {:016x}, but the active artifact has {:016x}",
                self.public_path.unwrap_or("<unknown>"),
                expected_fingerprint,
                actual_fingerprint
            ),
            BindingErrorKind::MissingType => write!(
                formatter,
                "generated binding type `{}` is missing from the active artifact",
                self.public_path.unwrap_or("<unknown>")
            ),
            BindingErrorKind::IncompatibleType {
                expected_fingerprint,
                actual_fingerprint,
            } => write!(
                formatter,
                "generated binding type `{}` has schema {:016x}, but the active artifact has {:016x}",
                self.public_path.unwrap_or("<unknown>"),
                expected_fingerprint,
                actual_fingerprint
            ),
            BindingErrorKind::ReentryUnavailable => {
                formatter.write_str("generated active binding requires a running Vela call session")
            }
        }
    }
}

impl std::error::Error for BindingError {}

impl From<BindingError> for vela_vm::error::VmError {
    fn from(error: BindingError) -> Self {
        vela_vm::error::VmError::new(vela_vm::error::VmErrorKind::TypeContractViolation {
            expected: "compatible generated Vela binding".to_owned(),
            actual: error.to_string(),
            debug_name: error.public_path.unwrap_or("generated bindings").to_owned(),
        })
        .with_source_span(error.source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingCallOptions {
    call: CallOptions,
}

impl BindingCallOptions {
    #[must_use]
    pub const fn new(call: CallOptions) -> Self {
        Self { call }
    }
}

impl Default for BindingCallOptions {
    fn default() -> Self {
        Self::new(CallOptions::new(
            DEFAULT_BINDING_EXECUTION_UNITS,
            DEFAULT_BINDING_MEMORY_BYTES,
            DEFAULT_BINDING_CALL_DEPTH,
        ))
    }
}

pub struct RootBinding<'runtime> {
    runtime: &'runtime mut Runtime,
    options: BindingCallOptions,
}

impl<'runtime> RootBinding<'runtime> {
    pub fn bind(
        runtime: &'runtime mut Runtime,
        expected: &'static BindingSchemaSpec,
    ) -> BindingResult<Self> {
        Self::bind_with_options(runtime, expected, BindingCallOptions::default())
    }

    pub fn bind_with_options(
        runtime: &'runtime mut Runtime,
        expected: &'static BindingSchemaSpec,
        options: BindingCallOptions,
    ) -> BindingResult<Self> {
        validate_schema(expected, runtime.active_binding_schema())?;
        Ok(Self { runtime, options })
    }
}

pub struct ActiveBinding<'binding, 'context> {
    context: &'binding mut NativeCallContext<'context>,
}

impl<'binding, 'context> ActiveBinding<'binding, 'context> {
    pub fn bind(
        context: &'binding mut NativeCallContext<'context>,
        expected: &'static BindingSchemaSpec,
    ) -> BindingResult<Self> {
        let actual = context.binding_schema().ok_or(BindingError {
            kind: BindingErrorKind::ReentryUnavailable,
            public_path: None,
            source: None,
        })?;
        validate_schema(expected, actual)?;
        Ok(Self { context })
    }
}

pub trait BindingAuthority {
    fn call<R, A>(&mut self, callable: &BindingCallable, args: A) -> VmResult<R>
    where
        R: FromScriptArg,
        A: IntoBindingArgs;

    fn call_prepared<R>(&mut self, callable: &BindingCallable, args: CallArgs) -> VmResult<R>
    where
        R: FromScriptArg;
}

impl BindingAuthority for RootBinding<'_> {
    fn call<R, A>(&mut self, callable: &BindingCallable, args: A) -> VmResult<R>
    where
        R: FromScriptArg,
        A: IntoBindingArgs,
    {
        let spec = callable.spec()?;
        let result = self.runtime.call(
            stable_target(spec),
            args.into_call_args(),
            self.options.call.clone(),
        )?;
        let owned = self.runtime.value_to_owned(&result)?;
        R::from_script_arg(&owned)
    }

    fn call_prepared<R>(&mut self, callable: &BindingCallable, args: CallArgs) -> VmResult<R>
    where
        R: FromScriptArg,
    {
        let spec = callable.spec()?;
        let result = self
            .runtime
            .call(stable_target(spec), args, self.options.call.clone())?;
        let owned = self.runtime.value_to_owned(&result)?;
        R::from_script_arg(&owned)
    }
}

impl BindingAuthority for ActiveBinding<'_, '_> {
    fn call<R, A>(&mut self, callable: &BindingCallable, args: A) -> VmResult<R>
    where
        R: FromScriptArg,
        A: IntoBindingArgs,
    {
        let spec = callable.spec()?;
        self.context.require_capabilities(
            spec.public_path,
            binding_required_capabilities(spec.effect_bits),
        )?;
        let result = self
            .context
            .call(stable_target(spec), args.into_call_args())?;
        let owned = self.context.value_to_owned(&result)?;
        R::from_script_arg(&owned)
    }

    fn call_prepared<R>(&mut self, callable: &BindingCallable, args: CallArgs) -> VmResult<R>
    where
        R: FromScriptArg,
    {
        let spec = callable.spec()?;
        self.context.require_capabilities(
            spec.public_path,
            binding_required_capabilities(spec.effect_bits),
        )?;
        let result = self.context.call(stable_target(spec), args)?;
        let owned = self.context.value_to_owned(&result)?;
        R::from_script_arg(&owned)
    }
}

pub trait IntoBindingArgs {
    fn into_call_args(self) -> CallArgs;
}

impl IntoBindingArgs for () {
    fn into_call_args(self) -> CallArgs {
        CallArgs::new()
    }
}

macro_rules! binding_tuple_args {
    ($(($($type:ident),+)),+ $(,)?) => {
        $(
            impl<$($type),+> IntoBindingArgs for ($($type,)+)
            where
                $($type: IntoScriptArg,)+
            {
                #[allow(non_snake_case)]
                fn into_call_args(self) -> CallArgs
                {
                    let ($($type,)+) = self;
                    CallArgs::from_positional([$($type.into_script_arg(),)+])
                }
            }
        )+
    };
}

binding_tuple_args!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
);

fn stable_target(spec: &'static BindingCallableSpec) -> StableVelaFunction {
    StableVelaFunction {
        function: FunctionId::new(spec.executable),
        diagnostic_name: spec.public_path.to_owned(),
    }
}

fn binding_required_capabilities(effect_bits: u32) -> CapabilitySet {
    let mut required = CapabilitySet::new();
    for (bit, capability) in [
        (6, Capability::ReflectionRead),
        (7, Capability::ReflectionWrite),
        (8, Capability::ReflectionCall),
        (9, Capability::EventEmit),
        (10, Capability::Time),
        (11, Capability::Random),
        (12, Capability::IoRead),
        (13, Capability::IoWrite),
        (14, Capability::HostRead),
        (15, Capability::HostWrite),
    ] {
        if effect_bits & (1 << bit) != 0 {
            required.insert(capability);
        }
    }
    if required.contains(Capability::HostWrite) {
        required = required.without(Capability::HostRead);
    }
    required
}

fn validate_schema(expected: &BindingSchemaSpec, actual: &RustBindingSchema) -> BindingResult<()> {
    if expected.version != actual.version() {
        return Err(BindingError {
            kind: BindingErrorKind::SchemaVersion {
                expected: expected.version,
                actual: actual.version(),
            },
            public_path: None,
            source: None,
        });
    }
    if expected.checksum == actual.checksum() {
        return Ok(());
    }
    for callable in expected.callables {
        let Some(actual_callable) = actual.callable(callable.identity.runtime()) else {
            return Err(BindingError {
                kind: BindingErrorKind::MissingCallable,
                public_path: Some(callable.public_path),
                source: Some(callable.source),
            });
        };
        if actual_callable.contract_fingerprint != callable.contract_fingerprint
            || actual_callable.executable.get() != callable.executable
        {
            return Err(BindingError {
                kind: BindingErrorKind::IncompatibleCallable {
                    expected_fingerprint: callable.contract_fingerprint,
                    actual_fingerprint: actual_callable.contract_fingerprint,
                },
                public_path: Some(callable.public_path),
                source: Some(callable.source),
            });
        }
    }
    for expected_type in expected.types {
        let Some(actual_type) = actual.type_definition(TypeId::new(expected_type.type_id)) else {
            return Err(BindingError {
                kind: BindingErrorKind::MissingType,
                public_path: Some(expected_type.public_path),
                source: Some(expected_type.source),
            });
        };
        let actual_fingerprint = match actual_type {
            vela_bytecode::RustBindingTypeDefinition::Record(record) => record.schema_fingerprint,
            vela_bytecode::RustBindingTypeDefinition::Enum(item) => item.schema_fingerprint,
        };
        if actual_fingerprint != expected_type.schema_fingerprint {
            return Err(BindingError {
                kind: BindingErrorKind::IncompatibleType {
                    expected_fingerprint: expected_type.schema_fingerprint,
                    actual_fingerprint,
                },
                public_path: Some(expected_type.public_path),
                source: Some(expected_type.source),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::IntoScriptArg;
    use crate::engine::Engine;
    use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, TypeHint};

    #[test]
    fn binding_effect_bits_match_the_current_value_only_schema() {
        for bits in 0_u32..(1 << 16) {
            let present = |index: u32| bits & (1_u32 << index) != 0_u32;
            let effects = vela_bytecode::RustBindingEffectSet {
                may_trap: present(0),
                may_allocate: present(1),
                script_call: present(2),
                dynamic_call: present(3),
                state_read: present(4),
                state_write: present(5),
                reflection_read: present(6),
                reflection_write: present(7),
                reflection_call: present(8),
                emits_event: present(9),
                reads_time: present(10),
                uses_random: present(11),
                reads_io: present(12),
                writes_io: present(13),
                host_read: present(14),
                host_write: present(15),
            };
            assert_eq!(
                binding_required_capabilities(effects.bits()),
                effects.required_capabilities()
            );
        }
    }

    fn function_spec(
        schema: &RustBindingSchema,
        path: &'static str,
        fingerprint_delta: u64,
        identity_delta: u128,
    ) -> &'static BindingSchemaSpec {
        let callable = schema.callables().next().expect("test callable");
        let RustBindingCallableIdentity::Function(identity) = callable.identity else {
            panic!("expected function binding")
        };
        let callables = Box::leak(
            vec![BindingCallableSpec::function(
                path,
                identity.get() ^ identity_delta,
                callable.executable.get(),
                callable.contract_fingerprint ^ fingerprint_delta,
                callable.effects.bits(),
                callable.source.source.get(),
                callable.source.start,
                callable.source.end,
            )]
            .into_boxed_slice(),
        );
        Box::leak(Box::new(BindingSchemaSpec::new(
            schema.version(),
            schema.checksum() ^ fingerprint_delta ^ u64::from(identity_delta != 0),
            callables,
        )))
    }

    fn function_and_type_spec(schema: &RustBindingSchema) -> &'static BindingSchemaSpec {
        let spec = function_spec(schema, "echo", 0, 0);
        let definition = schema.types().next().expect("test type");
        let (public_path, type_id, fingerprint, source) = match definition {
            vela_bytecode::RustBindingTypeDefinition::Record(record) => (
                record.public_path.clone(),
                record.type_id,
                record.schema_fingerprint,
                record.source,
            ),
            vela_bytecode::RustBindingTypeDefinition::Enum(item) => (
                item.public_path.clone(),
                item.type_id,
                item.schema_fingerprint,
                item.source,
            ),
        };
        let public_path = Box::leak(public_path.into_boxed_str());
        let types = Box::leak(
            vec![BindingTypeSpec::new(
                public_path,
                type_id.get(),
                fingerprint,
                source.source.get(),
                source.start,
                source.end,
            )]
            .into_boxed_slice(),
        );
        Box::leak(Box::new((*spec).with_types(types)))
    }

    #[test]
    fn root_binding_validates_once_and_calls_by_stable_function_id() {
        let engine = Engine::builder().build().expect("engine");
        let program = engine
            .compile_source("pub fn add(left: i64, right: i64) -> i64 { return left + right; }")
            .expect("program");
        let schema = program.binding_schema().clone();
        let expected = function_spec(&schema, "deliberately::not_the_runtime_name", 0, 0);
        let mut runtime = Runtime::new(engine, program).expect("runtime");
        let mut binding = RootBinding::bind(&mut runtime, expected).expect("compatible binding");
        let callable = BindingCallable::new(expected, 0);

        let result: i64 = binding
            .call(&callable, (20_i64, 22_i64))
            .expect("typed call");

        assert_eq!(result, 42);
    }

    #[test]
    fn binding_mismatch_reports_generated_source_and_contract() {
        let engine = Engine::builder().build().expect("engine");
        let program = engine
            .compile_source("pub fn value() -> i64 { return 1; }")
            .expect("program");
        let schema = program.binding_schema().clone();
        let source = schema.callables().next().expect("callable").source;
        let incompatible = function_spec(&schema, "value", 1, 0);
        let mut runtime = Runtime::new(engine, program).expect("runtime");

        let error = RootBinding::bind(&mut runtime, incompatible)
            .err()
            .expect("incompatible binding");

        assert_eq!(error.code(), "binding.callable.incompatible");
        assert_eq!(error.public_path, Some("value"));
        assert_eq!(error.source, Some(source));
        assert!(matches!(
            error.kind,
            BindingErrorKind::IncompatibleCallable { .. }
        ));
    }

    #[test]
    fn binding_missing_target_reports_generated_source() {
        let engine = Engine::builder().build().expect("engine");
        let program = engine
            .compile_source("pub fn value() -> i64 { return 1; }")
            .expect("program");
        let schema = program.binding_schema().clone();
        let source = schema.callables().next().expect("callable").source;
        let missing = function_spec(&schema, "value", 0, 1);
        let mut runtime = Runtime::new(engine, program).expect("runtime");

        let error = RootBinding::bind(&mut runtime, missing)
            .err()
            .expect("missing binding");

        assert_eq!(error.code(), "binding.callable.missing");
        assert_eq!(error.source, Some(source));
    }

    #[test]
    fn binding_rejects_incompatible_generated_model_schema() {
        let engine = Engine::builder().build().expect("engine");
        let first = engine
            .compile_source(
                "pub struct Model { value: i64 } pub fn echo(value: Model) -> Model { return value; }",
            )
            .expect("first program");
        let expected = function_and_type_spec(first.binding_schema());
        let changed = engine
            .compile_source(
                "pub struct Model { value: String } pub fn echo(value: Model) -> Model { return value; }",
            )
            .expect("changed program");
        let mut runtime = Runtime::new(engine, changed).expect("runtime");

        let error = RootBinding::bind(&mut runtime, expected)
            .err()
            .expect("model mismatch");

        assert_eq!(error.code(), "binding.type.incompatible");
        assert_eq!(error.public_path, Some("Model"));
        assert!(matches!(
            error.kind,
            BindingErrorKind::IncompatibleType { .. }
        ));
    }

    fn assert_active_binding_native_effect_ceiling(effects: EffectSet, capability: Capability) {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        const SOURCE: &str = "pub fn guarded() -> i64 { return test::value(); } fn main() -> i64 { return test::reenter(); }";
        for ceiling in [
            EffectSet::pure(),
            EffectSet::host_read(),
            EffectSet::host_write(),
        ] {
            for prepared in [false, true] {
                let calls = Arc::new(AtomicUsize::new(0));
                let builder = || {
                    let calls = Arc::clone(&calls);
                    Engine::builder()
                        .capability(Capability::HostRead)
                        .capability(Capability::HostWrite)
                        .register_native_fn(
                            NativeFunctionDesc::new("test::value", FunctionId::new(0xDCA1))
                                .returns(TypeHint::i64())
                                .effects(effects),
                            move |_| {
                                calls.fetch_add(1, Ordering::SeqCst);
                                Ok(OwnedValue::i64(42))
                            },
                        )
                };
                let descriptor = || {
                    NativeFunctionDesc::new("test::reenter", FunctionId::new(0xDCA2))
                        .returns(TypeHint::i64())
                        .effects(ceiling)
                };
                let preview_engine = builder()
                    .register_context_host_native_fn(descriptor(), |_, _| Ok(OwnedValue::i64(0)))
                    .build()
                    .expect("preview engine");
                let preview = preview_engine
                    .compile_source(SOURCE)
                    .expect("preview program");
                let guarded = preview
                    .binding_schema()
                    .callables()
                    .find(|callable| callable.public_path == "guarded")
                    .expect("guarded binding");
                assert_eq!(
                    guarded.effects.required_capabilities(),
                    CapabilitySet::new().with(capability)
                );
                let expected = function_spec(preview.binding_schema(), "guarded", 0, 0);
                let engine = builder()
                    .register_context_host_native_fn(descriptor(), move |_, context| {
                        let callable = BindingCallable::new(expected, 0);
                        let mut binding = ActiveBinding::bind(context, expected)?;
                        let value: i64 = if prepared {
                            binding.call_prepared(&callable, CallArgs::new())?
                        } else {
                            binding.call(&callable, ())?
                        };
                        Ok(OwnedValue::i64(value))
                    })
                    .build()
                    .expect("engine");
                let program = engine.compile_source(SOURCE).expect("program");
                let mut runtime = Runtime::new(engine, program).expect("runtime");
                let result = runtime.call(
                    "main",
                    CallArgs::new(),
                    CallOptions::new(10_000, 1 << 20, 32),
                );
                let allowed = ceiling.writes_host()
                    || (ceiling.reads_host() && capability == Capability::HostRead);
                if allowed {
                    assert_eq!(
                        runtime.value_to_owned(&result.expect("declared effect ceiling")),
                        Ok(OwnedValue::i64(42))
                    );
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                } else {
                    assert!(
                        matches!(result.expect_err("engine grant cannot expand a native effect ceiling").kind(),
                        VmErrorKind::PermissionDenied { native, capability: denied }
                            if native == "guarded" && denied == capability.as_str())
                    );
                    assert_eq!(calls.load(Ordering::SeqCst), 0);
                }
            }
        }
    }

    #[test]
    fn active_binding_enforces_value_native_host_write_effect_ceiling() {
        assert_active_binding_native_effect_ceiling(EffectSet::host_write(), Capability::HostWrite);
    }

    #[test]
    fn active_binding_enforces_value_native_host_read_effect_ceiling() {
        assert_active_binding_native_effect_ceiling(EffectSet::host_read(), Capability::HostRead);
    }

    #[test]
    fn active_binding_reenters_the_pinned_execution_session() {
        const SOURCE: &str = r#"
pub fn inner(value: i64) -> i64 { return value + 1; }
fn main(value: i64) -> i64 { return test::reenter(value); }
"#;
        fn descriptor() -> NativeFunctionDesc {
            NativeFunctionDesc::new("test::reenter", FunctionId::new(0xB1AD))
                .param("value", TypeHint::i64())
                .returns(TypeHint::i64())
                .effects(EffectSet::pure())
                .access(FunctionAccess::public())
        }

        let preview_engine = Engine::builder()
            .register_context_host_native_fn(descriptor(), |_args, _context| {
                Ok(0_i64.into_script_arg())
            })
            .build()
            .expect("preview engine");
        let preview = preview_engine
            .compile_source(SOURCE)
            .expect("preview program");
        let expected = function_spec(preview.binding_schema(), "inner", 0, 0);
        let callable = Box::leak(Box::new(BindingCallable::new(expected, 0)));

        let engine = Engine::builder()
            .register_context_host_native_fn(descriptor(), move |args, context| {
                let value = i64::from_script_arg(args.first().ok_or_else(|| {
                    vela_vm::error::VmError::new(
                        vela_vm::error::VmErrorKind::TypeContractViolation {
                            expected: "i64".to_owned(),
                            actual: "missing".to_owned(),
                            debug_name: "test::reenter".to_owned(),
                        },
                    )
                })?)?;
                let mut binding = ActiveBinding::bind(context, expected)?;
                let result: i64 = binding.call(callable, (value,))?;
                Ok(result.into_script_arg())
            })
            .build()
            .expect("engine");
        let program = engine.compile_source(SOURCE).expect("program");
        let mut runtime = Runtime::new(engine, program).expect("runtime");
        let result = runtime
            .call(
                "main",
                CallArgs::from_positional([41_i64.into_script_arg()]),
                CallOptions::new(10_000, 1024 * 1024, 32),
            )
            .expect("root call");
        let owned = runtime.value_to_owned(&result).expect("owned result");

        assert_eq!(i64::from_script_arg(&owned), Ok(42));
    }
}
