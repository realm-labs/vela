use std::sync::Arc;

use vela_common::{CallableAsyncness, CollectionViewMutation, PrimitiveTag, Span};
use vela_def::FunctionId;
use vela_reflect::registry::{AttrMap, TypeKey};
use vela_vm::AsyncDirectHostFunction;
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;
use crate::interop::CallableContract;
use crate::permission::{Capability, CapabilitySet};

pub type NativeFunctionId = FunctionId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFunctionDesc {
    pub id: NativeFunctionId,
    pub name: String,
    pub params: Vec<NativeParamDesc>,
    pub returns: TypeHint,
    pub effects: EffectSet,
    pub asyncness: CallableAsyncness,
    pub access: FunctionAccess,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
    pub callable_contract: Option<CallableContract>,
}

impl NativeFunctionDesc {
    #[must_use]
    pub fn new(name: impl Into<String>, id: NativeFunctionId) -> Self {
        Self {
            id,
            name: name.into(),
            params: Vec::new(),
            returns: TypeHint::Any,
            effects: EffectSet::default(),
            asyncness: CallableAsyncness::Sync,
            access: FunctionAccess::default(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
            callable_contract: None,
        }
    }

    #[must_use]
    pub fn param(mut self, name: impl Into<String>, hint: TypeHint) -> Self {
        self.params.push(NativeParamDesc {
            name: name.into(),
            hint,
        });
        self
    }

    #[must_use]
    pub fn returns(mut self, hint: TypeHint) -> Self {
        self.returns = hint;
        self
    }

    #[must_use]
    pub fn effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub fn asyncness(mut self, asyncness: CallableAsyncness) -> Self {
        self.asyncness = asyncness;
        self
    }

    #[must_use]
    pub fn access(mut self, access: FunctionAccess) -> Self {
        self.access = access;
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    #[must_use]
    pub fn callable_contract(mut self, callable_contract: CallableContract) -> Self {
        self.callable_contract = Some(callable_contract);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeParamDesc {
    pub name: String,
    pub hint: TypeHint,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet {
    bits: u16,
}

impl EffectSet {
    const READS_HOST: u16 = 1 << (Capability::HostRead as u8);
    const WRITES_HOST: u16 = 1 << (Capability::HostWrite as u8);
    const EMITS_EVENTS: u16 = 1 << (Capability::EventEmit as u8);
    const READS_TIME: u16 = 1 << (Capability::Time as u8);
    const USES_RANDOM: u16 = 1 << (Capability::Random as u8);
    const READS_IO: u16 = 1 << (Capability::IoRead as u8);
    const WRITES_IO: u16 = 1 << (Capability::IoWrite as u8);
    const READS_REFLECTION: u16 = 1 << (Capability::ReflectionRead as u8);
    const WRITES_REFLECTION: u16 = 1 << (Capability::ReflectionWrite as u8);
    const CALLS_REFLECTION: u16 = 1 << (Capability::ReflectionCall as u8);

    #[must_use]
    pub const fn pure() -> Self {
        Self { bits: 0 }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            bits: Self::READS_HOST,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            bits: Self::READS_HOST | Self::WRITES_HOST,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            bits: Self::EMITS_EVENTS,
        }
    }

    #[must_use]
    pub const fn time() -> Self {
        Self {
            bits: Self::READS_TIME,
        }
    }

    #[must_use]
    pub const fn random() -> Self {
        Self {
            bits: Self::USES_RANDOM,
        }
    }

    #[must_use]
    pub const fn io_read() -> Self {
        Self {
            bits: Self::READS_IO,
        }
    }

    #[must_use]
    pub const fn io_write() -> Self {
        Self {
            bits: Self::WRITES_IO,
        }
    }

    #[must_use]
    pub const fn reflection_read() -> Self {
        Self {
            bits: Self::READS_REFLECTION,
        }
    }

    #[must_use]
    pub const fn reflection_write() -> Self {
        Self {
            bits: Self::WRITES_REFLECTION,
        }
    }

    #[must_use]
    pub const fn reflection_call() -> Self {
        Self {
            bits: Self::CALLS_REFLECTION,
        }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Returns the normalized fixed-bit representation used by callable ABI
    /// fingerprints. Deployment grants are intentionally represented by
    /// [`CapabilitySet`] instead and never enter these bits.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.bits
    }

    /// Returns whether `self` is an effect ceiling that permits every effect
    /// in `required`.
    #[must_use]
    pub const fn contains_all(self, required: Self) -> bool {
        self.bits & required.bits == required.bits
    }

    #[must_use]
    pub const fn reads_host(self) -> bool {
        self.contains(Self::READS_HOST)
    }

    #[must_use]
    pub const fn writes_host(self) -> bool {
        self.contains(Self::WRITES_HOST)
    }

    #[must_use]
    pub const fn emits_events(self) -> bool {
        self.contains(Self::EMITS_EVENTS)
    }

    #[must_use]
    pub const fn reads_time(self) -> bool {
        self.contains(Self::READS_TIME)
    }

    #[must_use]
    pub const fn uses_random(self) -> bool {
        self.contains(Self::USES_RANDOM)
    }

    #[must_use]
    pub const fn reads_io(self) -> bool {
        self.contains(Self::READS_IO)
    }

    #[must_use]
    pub const fn writes_io(self) -> bool {
        self.contains(Self::WRITES_IO)
    }

    #[must_use]
    pub const fn reads_reflection(self) -> bool {
        self.contains(Self::READS_REFLECTION)
    }

    #[must_use]
    pub const fn writes_reflection(self) -> bool {
        self.contains(Self::WRITES_REFLECTION)
    }

    #[must_use]
    pub const fn calls_reflection(self) -> bool {
        self.contains(Self::CALLS_REFLECTION)
    }

    pub const fn required_capability_set(self) -> CapabilitySet {
        let mut capabilities = CapabilitySet::from_bits(self.bits as u64);
        if self.writes_host() {
            capabilities = capabilities.without(Capability::HostRead);
        }
        capabilities
    }

    pub fn required_capabilities(&self) -> impl Iterator<Item = Capability> {
        (*self).required_capability_set().iter()
    }

    const fn contains(self, bit: u16) -> bool {
        self.bits & bit != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAccess {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
}

impl FunctionAccess {
    #[must_use]
    pub fn public() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn private() -> Self {
        Self {
            public: false,
            reflect_visible: false,
            reflect_callable: false,
        }
    }

    #[must_use]
    pub fn reflect_callable(mut self, reflect_callable: bool) -> Self {
        self.reflect_callable = reflect_callable;
        self
    }

    #[must_use]
    pub fn reflect_visible(mut self, reflect_visible: bool) -> Self {
        self.reflect_visible = reflect_visible;
        self
    }
}

impl Default for FunctionAccess {
    fn default() -> Self {
        Self {
            public: true,
            reflect_visible: true,
            reflect_callable: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeHint {
    Any,
    Primitive(PrimitiveTag),
    Array,
    ArrayOf(Box<TypeHint>),
    ArrayViewOf(Box<TypeHint>),
    ArrayMutOf {
        element: Box<TypeHint>,
        mutation: CollectionViewMutation,
    },
    Map,
    MapOf {
        key: Box<TypeHint>,
        value: Box<TypeHint>,
    },
    MapViewOf {
        key: Box<TypeHint>,
        value: Box<TypeHint>,
    },
    MapMutOf {
        key: Box<TypeHint>,
        value: Box<TypeHint>,
        mutation: CollectionViewMutation,
    },
    Set,
    SetOf(Box<TypeHint>),
    SetViewOf(Box<TypeHint>),
    SetMutOf {
        element: Box<TypeHint>,
        mutation: CollectionViewMutation,
    },
    TupleOf(Vec<TypeHint>),
    Iterator,
    IteratorOf(Box<TypeHint>),
    OptionOf(Box<TypeHint>),
    ResultOf {
        ok: Box<TypeHint>,
        err: Box<TypeHint>,
    },
    PathProxy,
    Record(TypeKey),
    Enum(TypeKey),
    Host(TypeKey),
    Trait(String),
    Function,
}

impl TypeHint {
    #[must_use]
    pub const fn primitive(tag: PrimitiveTag) -> Self {
        Self::Primitive(tag)
    }

    #[must_use]
    pub const fn unit() -> Self {
        Self::Primitive(PrimitiveTag::Unit)
    }

    #[must_use]
    pub const fn boolean() -> Self {
        Self::Primitive(PrimitiveTag::Bool)
    }

    #[must_use]
    pub const fn char() -> Self {
        Self::Primitive(PrimitiveTag::Char)
    }

    #[must_use]
    pub const fn i8() -> Self {
        Self::Primitive(PrimitiveTag::I8)
    }

    #[must_use]
    pub const fn i16() -> Self {
        Self::Primitive(PrimitiveTag::I16)
    }

    #[must_use]
    pub const fn i32() -> Self {
        Self::Primitive(PrimitiveTag::I32)
    }

    #[must_use]
    pub const fn i64() -> Self {
        Self::Primitive(PrimitiveTag::I64)
    }

    #[must_use]
    pub const fn u8() -> Self {
        Self::Primitive(PrimitiveTag::U8)
    }

    #[must_use]
    pub const fn u16() -> Self {
        Self::Primitive(PrimitiveTag::U16)
    }

    #[must_use]
    pub const fn u32() -> Self {
        Self::Primitive(PrimitiveTag::U32)
    }

    #[must_use]
    pub const fn u64() -> Self {
        Self::Primitive(PrimitiveTag::U64)
    }

    #[must_use]
    pub const fn f32() -> Self {
        Self::Primitive(PrimitiveTag::F32)
    }

    #[must_use]
    pub const fn f64() -> Self {
        Self::Primitive(PrimitiveTag::F64)
    }

    #[must_use]
    pub const fn string() -> Self {
        Self::Primitive(PrimitiveTag::String)
    }

    #[must_use]
    pub const fn bytes() -> Self {
        Self::Primitive(PrimitiveTag::Bytes)
    }

    #[must_use]
    pub const fn iterator() -> Self {
        Self::Iterator
    }

    #[must_use]
    pub fn array_of(element: TypeHint) -> Self {
        Self::ArrayOf(Box::new(element))
    }

    #[must_use]
    pub fn array_view_of(element: TypeHint) -> Self {
        Self::ArrayViewOf(Box::new(element))
    }

    #[must_use]
    pub fn array_mut_of(element: TypeHint, mutation: CollectionViewMutation) -> Self {
        Self::ArrayMutOf {
            element: Box::new(element),
            mutation,
        }
    }

    #[must_use]
    pub fn map_of(key: TypeHint, value: TypeHint) -> Self {
        Self::MapOf {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    #[must_use]
    pub fn map_view_of(key: TypeHint, value: TypeHint) -> Self {
        Self::MapViewOf {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    #[must_use]
    pub fn map_mut_of(key: TypeHint, value: TypeHint, mutation: CollectionViewMutation) -> Self {
        Self::MapMutOf {
            key: Box::new(key),
            value: Box::new(value),
            mutation,
        }
    }

    #[must_use]
    pub fn set_of(element: TypeHint) -> Self {
        Self::SetOf(Box::new(element))
    }

    #[must_use]
    pub fn set_view_of(element: TypeHint) -> Self {
        Self::SetViewOf(Box::new(element))
    }

    #[must_use]
    pub fn set_mut_of(element: TypeHint, mutation: CollectionViewMutation) -> Self {
        Self::SetMutOf {
            element: Box::new(element),
            mutation,
        }
    }

    #[must_use]
    pub fn tuple_of(elements: impl IntoIterator<Item = TypeHint>) -> Self {
        Self::TupleOf(elements.into_iter().collect())
    }

    #[must_use]
    pub fn iterator_of(item: TypeHint) -> Self {
        Self::IteratorOf(Box::new(item))
    }

    #[must_use]
    pub fn option_of(payload: TypeHint) -> Self {
        Self::OptionOf(Box::new(payload))
    }

    #[must_use]
    pub fn result_of(ok: TypeHint, err: TypeHint) -> Self {
        Self::ResultOf {
            ok: Box::new(ok),
            err: Box::new(err),
        }
    }
}

pub type NativeFunction =
    Arc<dyn Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static>;

/// The lifetime-erased future returned by one registered async Rust call.
///
/// The future may borrow its invocation arguments, but it must remain `Send`
/// for that scoped lifetime.
///
/// ```compile_fail
/// use std::rc::Rc;
/// use vela_engine::native::NativeCallFuture;
/// use vela_vm::owned_value::OwnedValue;
///
/// fn non_send<'call>(value: &'call Rc<i64>) -> NativeCallFuture<'call> {
///     Box::pin(async move {
///         let value = **value;
///         Ok(OwnedValue::i64(value))
///     })
/// }
/// ```
pub type NativeCallFuture<'call> = vela_vm::NativeCallFuture<'call>;

/// A `Send + Sync + 'static` factory whose returned future is scoped to one
/// invocation.
///
/// ```compile_fail
/// use std::rc::Rc;
/// use std::sync::Arc;
/// use vela_engine::native::{AsyncNativeFunction, NativeCallFuture};
/// use vela_vm::owned_value::OwnedValue;
///
/// let captured = Rc::new(1_i64);
/// let _factory: AsyncNativeFunction = Arc::new(move |_args| {
///     let captured = Rc::clone(&captured);
///     Box::pin(async move { Ok(OwnedValue::i64(*captured)) }) as NativeCallFuture<'_>
/// });
/// ```
pub type AsyncNativeFunction = vela_vm::AsyncNativeFunction;
pub type AsyncHostNativeFunction = Arc<
    dyn for<'call, 'host> Fn(
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
>;
pub type AsyncContextHostNativeFunction = Arc<
    dyn for<'call, 'host> Fn(
            &'call [OwnedValue],
            &'call mut NativeCallContext<'call, 'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
>;
pub type HostNativeFunction = Arc<
    dyn for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
>;
pub type ContextHostNativeFunction = Arc<
    dyn for<'ctx, 'host> Fn(
            &[OwnedValue],
            &mut NativeCallContext<'ctx, 'host>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
pub struct NativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: NativeFunction,
}

impl NativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        function: impl Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) -> Self {
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[derive(Clone)]
pub struct AsyncNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: AsyncNativeFunction,
}

impl AsyncNativeFunctionEntry {
    #[must_use]
    pub fn new(
        mut desc: NativeFunctionDesc,
        function: impl for<'call> Fn(&'call [OwnedValue]) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        desc.asyncness = CallableAsyncness::Async;
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[derive(Clone)]
pub struct AsyncHostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: AsyncHostNativeFunction,
}

pub type HostLeaseRequestFactory = Arc<
    dyn Fn(&[OwnedValue]) -> VmResult<vela_host::lease::HostLeaseRequestSet>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
pub struct AsyncDirectHostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub requests: HostLeaseRequestFactory,
    pub function: AsyncDirectHostFunction,
}

#[derive(Clone)]
pub struct ScopedHostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub requests: HostLeaseRequestFactory,
    pub function: ScopedHostNativeFunction,
}

pub type ScopedHostNativeFunction = Arc<
    dyn for<'host> Fn(
            &mut [vela_host::lease::ErasedHostLease<'host>],
            Vec<OwnedValue>,
        ) -> VmResult<ScopedHostNativeOutcome<'host>>
        + Send
        + Sync
        + 'static,
>;

pub enum ScopedHostNativeOutcome<'host> {
    Direct(vela_host::adapter::ScopedHostReturn<'host>),
    OptionSome(vela_host::adapter::ScopedHostReturn<'host>),
    ResultOk(vela_host::adapter::ScopedHostReturn<'host>),
    Tuple(vela_host::adapter::ScopedHostReturnGroup<'host>),
    OptionSomeTuple(vela_host::adapter::ScopedHostReturnGroup<'host>),
    ResultOkTuple(vela_host::adapter::ScopedHostReturnGroup<'host>),
    Value(OwnedValue),
}

impl ScopedHostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        requests: impl Fn(&[OwnedValue]) -> VmResult<vela_host::lease::HostLeaseRequestSet>
        + Send
        + Sync
        + 'static,
        function: impl for<'host> Fn(
            &mut [vela_host::lease::ErasedHostLease<'host>],
            Vec<OwnedValue>,
        ) -> VmResult<ScopedHostNativeOutcome<'host>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            desc,
            requests: Arc::new(requests),
            function: Arc::new(function),
        }
    }
}

impl AsyncDirectHostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        mut desc: NativeFunctionDesc,
        requests: impl Fn(&[OwnedValue]) -> VmResult<vela_host::lease::HostLeaseRequestSet>
        + Send
        + Sync
        + 'static,
        function: impl for<'invoke, 'lease> Fn(
            &'invoke mut [vela_host::lease::ErasedHostLease<'lease>],
            Vec<OwnedValue>,
        ) -> NativeCallFuture<'invoke>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        desc.asyncness = CallableAsyncness::Async;
        Self {
            desc,
            requests: Arc::new(requests),
            function: Arc::new(function),
        }
    }
}

impl AsyncHostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        mut desc: NativeFunctionDesc,
        function: impl for<'call, 'host> Fn(
            &'call [OwnedValue],
            &'call mut HostExecution<'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        desc.asyncness = CallableAsyncness::Async;
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[derive(Clone)]
pub struct AsyncContextHostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: AsyncContextHostNativeFunction,
}

impl AsyncContextHostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        mut desc: NativeFunctionDesc,
        function: impl for<'call, 'host> Fn(
            &'call [OwnedValue],
            &'call mut NativeCallContext<'call, 'host>,
        ) -> NativeCallFuture<'call>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        desc.asyncness = CallableAsyncness::Async;
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[derive(Clone)]
pub struct HostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: HostNativeFunction,
}

impl HostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        function: impl for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[derive(Clone)]
pub struct ContextHostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: ContextHostNativeFunction,
}

impl ContextHostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        function: impl for<'ctx, 'host> Fn(
            &[OwnedValue],
            &mut NativeCallContext<'ctx, 'host>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vela_vm::owned_value::OwnedValue;

    use super::{AsyncNativeFunction, EffectSet, NativeCallFuture};
    use crate::permission::Capability;

    fn require_send<T: Send>(_: T) {}

    fn borrowed_async_native(args: &[OwnedValue]) -> NativeCallFuture<'_> {
        Box::pin(async move { Ok(args.first().cloned().unwrap_or(OwnedValue::Unit)) })
    }

    #[test]
    fn async_native_factory_is_static_and_returns_scoped_send_future() {
        let factory: AsyncNativeFunction = Arc::new(borrowed_async_native);
        let args = [OwnedValue::i64(42)];

        require_send(factory(&args));
    }

    #[test]
    fn required_capability_set_matches_effect_flags() {
        let effects = EffectSet::host_write()
            .union(EffectSet::time())
            .union(EffectSet::io_write())
            .union(EffectSet::reflection_read())
            .union(EffectSet::reflection_call());

        let capabilities = effects.required_capability_set();

        assert!(effects.reads_host());
        assert!(effects.writes_host());
        assert!(!capabilities.contains(Capability::HostRead));
        assert!(capabilities.contains(Capability::HostWrite));
        assert!(capabilities.contains(Capability::Time));
        assert!(capabilities.contains(Capability::IoWrite));
        assert!(capabilities.contains(Capability::ReflectionRead));
        assert!(capabilities.contains(Capability::ReflectionCall));
        assert_eq!(
            effects.required_capabilities().collect::<Vec<_>>(),
            capabilities.iter().collect::<Vec<_>>()
        );
    }
}
