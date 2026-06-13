use std::sync::Arc;

use vela_common::PrimitiveTag;
use vela_common::Span;
use vela_def::FunctionId;
use vela_reflect::registry::{AttrMap, TypeKey};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;
use crate::permission::{Capability, CapabilitySet};

pub type NativeFunctionId = FunctionId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFunctionDesc {
    pub id: NativeFunctionId,
    pub name: String,
    pub params: Vec<NativeParamDesc>,
    pub returns: TypeHint,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
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
            access: FunctionAccess::default(),
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
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
    Map,
    Set,
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
    pub const fn null() -> Self {
        Self::Primitive(PrimitiveTag::Null)
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
}

pub type NativeFunction =
    Arc<dyn Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static>;
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
    use super::EffectSet;
    use crate::permission::Capability;

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
