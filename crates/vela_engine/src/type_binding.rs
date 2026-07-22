//! Unified Rust type registration and its sealed Engine snapshot.

use std::any::{Any, TypeId as RustTypeId};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;

use vela_common::{
    InteropTypeId, ReceiverCapabilities, ReceiverCapability, StoragePolicy, TypeAbiFingerprint,
    TypeBindingRegistryChecksum, stable_id,
};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_reflect::type_binding::TypeBindingDesc;

use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::host_type::HostTypeSpec;
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{HostNativeFunctionEntry, NativeFunctionDesc, TypeHint};
use crate::typed::TypedNativeMethodFunction;
use crate::{args::FromScriptArg, args::IntoScriptArg};
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

#[derive(Clone)]
pub struct TypeBinding<T: 'static> {
    spec: HostTypeSpec,
    storage: StoragePolicy,
    capabilities: ReceiverCapabilities,
    value_codec: Option<ValueCodec<T>>,
    constructors: Vec<TypeConstructorEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstructorStorage {
    Value,
    Host,
}

#[derive(Clone)]
struct TypeConstructorEntry {
    storage: ConstructorStorage,
    native: HostNativeFunctionEntry,
}

impl<T: 'static> TypeBinding<T> {
    #[must_use]
    pub fn value(type_desc: TypeDesc) -> Self
    where
        T: IntoScriptArg + FromScriptArg,
    {
        Self::value_with_codec(type_desc, ValueCodec::structural())
    }

    #[must_use]
    pub fn value_with_codec(type_desc: TypeDesc, codec: ValueCodec<T>) -> Self {
        Self {
            spec: HostTypeSpec::new(type_desc),
            storage: StoragePolicy::Value,
            capabilities: ReceiverCapabilities::OWNED_VALUE,
            value_codec: Some(codec),
            constructors: Vec::new(),
        }
    }

    #[must_use]
    pub fn host(type_desc: TypeDesc) -> Self {
        Self {
            spec: HostTypeSpec::new(type_desc),
            storage: StoragePolicy::Host,
            capabilities: ReceiverCapabilities::HOST_OBJECT,
            value_codec: None,
            constructors: Vec::new(),
        }
    }

    #[must_use]
    pub fn type_desc(&self) -> &TypeDesc {
        self.spec.type_desc()
    }

    #[must_use]
    pub const fn storage(&self) -> StoragePolicy {
        self.storage
    }

    #[must_use]
    pub const fn capabilities(&self) -> ReceiverCapabilities {
        self.capabilities
    }

    #[must_use]
    pub const fn receiver_capabilities(mut self, capabilities: ReceiverCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    #[must_use]
    pub fn method_desc(mut self, desc: NativeMethodDesc) -> Self {
        self.spec = self.spec.method_desc(desc);
        self
    }

    /// Registers a constructor owned by this binding.
    ///
    /// The callable is published through the ordinary native-function
    /// registry. Its qualified name must be directly below the bound type,
    /// for example `host::Widget::new`, and its declared return type must be
    /// this exact Value binding.
    #[must_use]
    pub fn constructor_fn(
        mut self,
        desc: NativeFunctionDesc,
        function: impl for<'host> Fn(
            &[OwnedValue],
            &mut vela_vm::HostExecution<'host>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.capabilities = self.capabilities.with(ReceiverCapability::Construct);
        self.constructors.push(TypeConstructorEntry {
            storage: ConstructorStorage::Value,
            native: HostNativeFunctionEntry::new(desc, function),
        });
        self
    }

    /// Registers a factory that transfers its Rust result into Runtime-owned
    /// host storage and returns only a `HostRef` to Vela.
    #[must_use]
    pub fn host_constructor_fn(
        mut self,
        desc: NativeFunctionDesc,
        factory: impl for<'host> Fn(&[OwnedValue], &mut vela_vm::HostExecution<'host>) -> VmResult<T>
        + Send
        + Sync
        + 'static,
    ) -> Self
    where
        T: vela_host::object::ScriptHostObject + Send + Sync,
    {
        let expected_type = self.spec.type_desc().host_type_id;
        self.capabilities = self.capabilities.with(ReceiverCapability::Construct);
        self.constructors.push(TypeConstructorEntry {
            storage: ConstructorStorage::Host,
            native: HostNativeFunctionEntry::new(desc, move |args, host| {
                let object = factory(args, host)?;
                let actual = object.host_type_id();
                let expected = expected_type
                    .expect("host constructor executes only after its Host TypeBinding seals");
                if actual != expected {
                    return Err(vela_host::error::HostError {
                        kind: vela_host::error::HostErrorKind::TypeMismatch { expected, actual },
                        source_span: None,
                    }
                    .into());
                }
                let root = host.adapter.retain_owned_host(Box::new(object))?;
                Ok(OwnedValue::HostRef(root))
            }),
        });
        self
    }

    #[must_use]
    pub fn native_method_fn(
        mut self,
        desc: NativeMethodDesc,
        function: impl for<'host> Fn(
            &vela_host::path::HostPath,
            &[vela_vm::owned_value::OwnedValue],
            &mut vela_vm::HostExecution<'host>,
        )
            -> vela_vm::error::VmResult<vela_vm::owned_value::OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.spec = self.spec.native_method_fn(desc, function);
        self
    }

    #[must_use]
    pub fn typed_native_method_fn<Args, F>(self, desc: NativeMethodDesc, function: F) -> Self
    where
        F: TypedNativeMethodFunction<Args>,
    {
        self.native_method_fn(desc, move |receiver, args, host| {
            function.call_method(receiver, args, host)
        })
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        TypeBindingRegistration,
        TypeDesc,
        Vec<NativeMethodDesc>,
        Vec<NativeMethodEntry>,
        Vec<HostNativeFunctionEntry>,
    ) {
        let storage = self.storage;
        let capabilities = self.capabilities;
        let constructors = self.constructors;
        let (type_desc, method_metadata, native_methods) = self.spec.into_parts();
        let registration = TypeBindingRegistration {
            key: type_desc.key.clone(),
            storage,
            capabilities,
            rust_type_id: None,
            value_codec: self.value_codec.map(ErasedValueCodec::new),
            constructors: constructors
                .iter()
                .map(|entry| TypeConstructorRegistration {
                    storage: entry.storage,
                    desc: entry.native.desc.clone(),
                })
                .collect(),
        };
        (
            registration,
            type_desc,
            method_metadata,
            native_methods,
            constructors.into_iter().map(|entry| entry.native).collect(),
        )
    }
}

pub struct ValueCodec<T> {
    encode: fn(T) -> OwnedValue,
    decode: fn(&OwnedValue) -> VmResult<T>,
    marker: PhantomData<fn(T) -> T>,
}

impl<T> ValueCodec<T> {
    #[must_use]
    pub const fn new(encode: fn(T) -> OwnedValue, decode: fn(&OwnedValue) -> VmResult<T>) -> Self {
        Self {
            encode,
            decode,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub fn encode(self, value: T) -> OwnedValue {
        (self.encode)(value)
    }

    pub fn decode(self, value: &OwnedValue) -> VmResult<T> {
        (self.decode)(value)
    }
}

impl<T> ValueCodec<T>
where
    T: IntoScriptArg + FromScriptArg,
{
    #[must_use]
    pub const fn structural() -> Self {
        Self::new(T::into_script_arg, T::from_script_arg)
    }
}

impl<T> Clone for ValueCodec<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ValueCodec<T> {}

#[derive(Clone)]
struct ErasedValueCodec(Arc<dyn Any + Send + Sync>);

impl ErasedValueCodec {
    fn new<T: 'static>(codec: ValueCodec<T>) -> Self {
        Self(Arc::new(codec))
    }

    fn get<T: 'static>(&self) -> Option<ValueCodec<T>> {
        self.0.downcast_ref::<ValueCodec<T>>().copied()
    }
}

#[derive(Clone)]
pub(crate) struct TypeBindingRegistration {
    rust_type_id: Option<RustTypeId>,
    key: TypeKey,
    storage: StoragePolicy,
    capabilities: ReceiverCapabilities,
    value_codec: Option<ErasedValueCodec>,
    constructors: Vec<TypeConstructorRegistration>,
}

#[derive(Clone)]
struct TypeConstructorRegistration {
    storage: ConstructorStorage,
    desc: NativeFunctionDesc,
}

impl TypeBindingRegistration {
    pub(crate) fn bind_rust_type<T: 'static>(&mut self) {
        self.rust_type_id = Some(RustTypeId::of::<T>());
    }
}

#[derive(Clone)]
pub struct TypeBindingRegistry {
    by_id: BTreeMap<InteropTypeId, TypeBindingDesc>,
    by_rust_type: HashMap<RustTypeId, InteropTypeId>,
    value_codecs_by_rust_type: HashMap<RustTypeId, ErasedValueCodec>,
    checksum: TypeBindingRegistryChecksum,
}

impl TypeBindingRegistry {
    pub(crate) fn seal(
        registrations: Vec<TypeBindingRegistration>,
        types: &[TypeDesc],
    ) -> EngineResult<Self> {
        let types = types
            .iter()
            .map(|desc| (desc.key.clone(), desc))
            .collect::<BTreeMap<_, _>>();
        let mut by_id = BTreeMap::new();
        let mut by_rust_type = HashMap::new();
        let mut value_codecs_by_rust_type = HashMap::new();
        for registration in registrations {
            let desc = types
                .get(&registration.key)
                .copied()
                .expect("registered type binding descriptor survived type validation");
            validate_representation(&registration, desc)?;
            let id = InteropTypeId::from_type_id(registration.key.id);
            let fingerprint = type_abi_fingerprint(
                id,
                registration.storage,
                registration.capabilities,
                &registration.constructors,
                desc,
            );
            let mut constructor_ids = registration
                .constructors
                .iter()
                .map(|constructor| constructor.desc.id)
                .collect::<Vec<_>>();
            constructor_ids.sort_unstable();
            let binding = TypeBindingDesc::new(
                id,
                registration.key.clone(),
                registration.storage,
                registration.capabilities,
                constructor_ids,
                fingerprint,
            );
            if by_id.insert(id, binding).is_some() {
                return Err(EngineError::new(EngineErrorKind::DuplicateInteropTypeId {
                    id: id.get(),
                }));
            }
            let rust_type_id = registration
                .rust_type_id
                .expect("EngineBuilder binds every pending registration to a Rust type");
            if let Some(existing) = by_rust_type.insert(rust_type_id, id) {
                return Err(EngineError::new(
                    EngineErrorKind::DuplicateRustTypeBinding {
                        first: existing.get(),
                        second: id.get(),
                    },
                ));
            }
            if let Some(codec) = registration.value_codec {
                value_codecs_by_rust_type.insert(rust_type_id, codec);
            }
        }
        let canonical = by_id
            .values()
            .map(|binding| {
                format!(
                    "{:032x}:{:016x}",
                    binding.id.get(),
                    binding.abi_fingerprint.get()
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        let checksum = TypeBindingRegistryChecksum::new(stable_id(
            "vela_type_binding_registry_v1",
            "",
            &canonical,
        ));
        Ok(Self {
            by_id,
            by_rust_type,
            value_codecs_by_rust_type,
            checksum,
        })
    }

    #[must_use]
    pub fn get(&self, id: InteropTypeId) -> Option<&TypeBindingDesc> {
        self.by_id.get(&id)
    }

    #[must_use]
    pub fn get_for<T: 'static>(&self) -> Option<&TypeBindingDesc> {
        let id = self.by_rust_type.get(&RustTypeId::of::<T>())?;
        self.by_id.get(id)
    }

    #[must_use]
    pub fn value_codec<T: 'static>(&self) -> Option<ValueCodec<T>> {
        self.value_codecs_by_rust_type
            .get(&RustTypeId::of::<T>())?
            .get::<T>()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TypeBindingDesc> {
        self.by_id.values()
    }

    #[must_use]
    pub const fn checksum(&self) -> TypeBindingRegistryChecksum {
        self.checksum
    }
}

fn validate_representation(
    registration: &TypeBindingRegistration,
    desc: &TypeDesc,
) -> EngineResult<()> {
    let valid_storage = match registration.storage {
        StoragePolicy::Value => desc.host_type_id.is_none() && registration.value_codec.is_some(),
        StoragePolicy::Host => {
            desc.host_type_id.is_some()
                && desc.kind == TypeKind::Host
                && registration.value_codec.is_none()
        }
    };
    if !valid_storage {
        return Err(EngineError::new(
            EngineErrorKind::InvalidTypeBindingStorage {
                name: desc.key.name.clone(),
                storage: registration.storage.as_str().to_owned(),
            },
        ));
    }
    if !registration
        .capabilities
        .contains(ReceiverCapability::Owned)
        || (registration
            .capabilities
            .contains(ReceiverCapability::Exclusive)
            && !registration
                .capabilities
                .contains(ReceiverCapability::Shared))
        || registration
            .capabilities
            .contains(ReceiverCapability::Construct)
            != !registration.constructors.is_empty()
    {
        return Err(EngineError::new(
            EngineErrorKind::InvalidTypeBindingCapabilities {
                name: desc.key.name.clone(),
                bits: registration.capabilities.bits(),
            },
        ));
    }
    validate_constructors(registration, desc)?;
    for method in &desc.methods {
        let valid = match method.receiver {
            ReceiverCapability::Owned => registration
                .capabilities
                .contains(ReceiverCapability::Owned),
            ReceiverCapability::Shared => registration
                .capabilities
                .contains(ReceiverCapability::Shared),
            ReceiverCapability::Exclusive => registration
                .capabilities
                .contains(ReceiverCapability::Exclusive),
            ReceiverCapability::Construct => false,
        };
        if !valid {
            return Err(EngineError::new(
                EngineErrorKind::InvalidTypeBindingMethodReceiver {
                    type_name: desc.key.name.clone(),
                    method: method.name.clone(),
                    receiver: method.receiver.as_str().to_owned(),
                },
            ));
        }
    }
    Ok(())
}

fn type_abi_fingerprint(
    id: InteropTypeId,
    storage: StoragePolicy,
    capabilities: ReceiverCapabilities,
    constructors: &[TypeConstructorRegistration],
    desc: &TypeDesc,
) -> TypeAbiFingerprint {
    let mut parts = vec![
        format!("id={:032x}", id.get()),
        format!("path={}", desc.key.name),
        format!("storage={}", storage.as_str()),
        format!("capabilities={:02x}", capabilities.bits()),
        format!("kind={}", type_kind_name(desc.kind)),
        format!(
            "schema={:016x}",
            desc.schema_hash
                .map_or(0, vela_reflect::registry::SchemaHash::get)
        ),
        format!(
            "host={:016x}",
            desc.host_type_id.map_or(0, vela_common::HostTypeId::get)
        ),
    ];
    let mut constructors = constructors.iter().collect::<Vec<_>>();
    constructors.sort_by_key(|constructor| constructor.desc.id);
    for constructor in constructors {
        parts.push(constructor_abi(&constructor.desc));
        parts.extend(
            constructor
                .desc
                .params
                .iter()
                .enumerate()
                .map(|(index, param)| {
                    format!(
                        "constructor-param={index}:{}:{}",
                        param.name,
                        crate::metadata::type_hint_display(&param.hint)
                    )
                }),
        );
    }
    let mut fields = desc.fields.iter().collect::<Vec<_>>();
    fields.sort_by_key(|field| field.id);
    parts.extend(fields.into_iter().map(field_abi));
    let mut methods = desc.methods.iter().collect::<Vec<_>>();
    methods.sort_by_key(|method| method.id);
    for method in methods {
        parts.push(format!(
            "method={:032x}:{}:{}:{}:{}:{}:{}",
            method.id.get(),
            method.name,
            method.asyncness.is_async(),
            method.receiver.as_str(),
            method.access.public,
            method_effect_bits(&method.effects),
            method.return_type.as_deref().unwrap_or("")
        ));
        parts.extend(method.params.iter().enumerate().map(|(index, param)| {
            format!(
                "method-param={index}:{}:{}:{}",
                param.name,
                param.type_hint.as_deref().unwrap_or(""),
                param.has_default
            )
        }));
    }
    let mut variants = desc.variants.iter().collect::<Vec<_>>();
    variants.sort_by_key(|variant| variant.id);
    for variant in variants {
        parts.push(format!(
            "variant={:032x}:{}",
            variant.id.get(),
            variant.name
        ));
        let mut fields = variant.fields.iter().collect::<Vec<_>>();
        fields.sort_by_key(|field| field.id);
        parts.extend(fields.into_iter().map(field_abi));
    }
    let mut traits = desc.traits.iter().collect::<Vec<_>>();
    traits.sort_by_key(|trait_desc| trait_desc.id);
    for trait_desc in traits {
        parts.push(format!(
            "trait={:032x}:{}",
            trait_desc.id.get(),
            trait_desc.name
        ));
        let mut methods = trait_desc.methods.iter().collect::<Vec<_>>();
        methods.sort_by_key(|method| method.id);
        for method in methods {
            parts.push(format!(
                "trait-method={:032x}:{}:{}:{}:{}",
                method.id.get(),
                method.name,
                method.asyncness.is_async(),
                method.has_default,
                method.return_type.as_deref().unwrap_or("")
            ));
            parts.extend(method.params.iter().enumerate().map(|(index, param)| {
                format!(
                    "trait-param={index}:{}:{}:{}",
                    param.name,
                    param.type_hint.as_deref().unwrap_or(""),
                    param.has_default
                )
            }));
        }
    }
    if let Some(index) = &desc.index_capability {
        parts.push(format!(
            "index={}:{}:{}:{}:{}:{}",
            index.readable,
            index.writable,
            index.addable,
            index.removable,
            index.key_type.as_deref().unwrap_or(""),
            index.value_type.as_deref().unwrap_or("")
        ));
    }
    TypeAbiFingerprint::new(stable_id("vela_type_binding_abi_v1", "", &parts.join("|")))
}

fn validate_constructors(
    registration: &TypeBindingRegistration,
    desc: &TypeDesc,
) -> EngineResult<()> {
    for constructor in &registration.constructors {
        let constructor_desc = &constructor.desc;
        let valid_owner = constructor_desc
            .name
            .rsplit_once("::")
            .is_some_and(|(owner, name)| {
                owner == desc.key.name && !name.is_empty() && !name.contains("::")
            });
        if !valid_owner {
            return Err(invalid_constructor(
                desc,
                constructor_desc,
                "qualified name is not directly owned by the bound type",
            ));
        }
        if constructor_desc.asyncness.is_async() {
            return Err(invalid_constructor(
                desc,
                constructor_desc,
                "constructors must be synchronous",
            ));
        }
        let expected = match (registration.storage, constructor.storage, desc.kind) {
            (StoragePolicy::Value, ConstructorStorage::Value, TypeKind::ScriptStruct) => {
                TypeHint::Record(desc.key.clone())
            }
            (StoragePolicy::Value, ConstructorStorage::Value, TypeKind::ScriptEnum) => {
                TypeHint::Enum(desc.key.clone())
            }
            (StoragePolicy::Value, ConstructorStorage::Value, _) => {
                return Err(invalid_constructor(
                    desc,
                    constructor_desc,
                    "Value constructors require a script struct or enum representation",
                ));
            }
            (StoragePolicy::Host, ConstructorStorage::Host, TypeKind::Host) => {
                TypeHint::Host(desc.key.clone())
            }
            (StoragePolicy::Value, ConstructorStorage::Host, _) => {
                return Err(invalid_constructor(
                    desc,
                    constructor_desc,
                    "a host factory cannot construct a Value binding",
                ));
            }
            (StoragePolicy::Host, ConstructorStorage::Value, _) => {
                return Err(invalid_constructor(
                    desc,
                    constructor_desc,
                    "a Host binding requires a host-owned factory",
                ));
            }
            (StoragePolicy::Host, ConstructorStorage::Host, _) => {
                unreachable!("Host storage validation requires a host type representation")
            }
        };
        if constructor_desc.returns != expected {
            return Err(invalid_constructor(
                desc,
                constructor_desc,
                "declared return type is not the bound type",
            ));
        }
    }
    Ok(())
}

fn invalid_constructor(
    desc: &TypeDesc,
    constructor: &NativeFunctionDesc,
    reason: &'static str,
) -> EngineError {
    EngineError::new(EngineErrorKind::InvalidTypeBindingConstructor {
        type_name: desc.key.name.clone(),
        constructor: constructor.name.clone(),
        reason,
    })
}

fn constructor_abi(constructor: &NativeFunctionDesc) -> String {
    format!(
        "constructor={:032x}:{}:{}:{}:{}:{}:{}:{}",
        constructor.id.get(),
        constructor.name,
        constructor.asyncness.is_async(),
        constructor.effects.bits(),
        constructor.access.public,
        constructor.access.reflect_visible,
        constructor.access.reflect_callable,
        crate::metadata::type_hint_display(&constructor.returns)
    )
}

fn field_abi(field: &FieldDesc) -> String {
    format!(
        "field={:032x}:{}:{}:{}:{}:{}",
        field.id.get(),
        field.name,
        field.type_hint.as_deref().unwrap_or(""),
        field.has_default,
        field.access.readable,
        field.access.writable
    )
}

const fn type_kind_name(kind: TypeKind) -> &'static str {
    match kind {
        TypeKind::Unit => "unit",
        TypeKind::Bool => "bool",
        TypeKind::I8 => "i8",
        TypeKind::I16 => "i16",
        TypeKind::I32 => "i32",
        TypeKind::I64 => "i64",
        TypeKind::U8 => "u8",
        TypeKind::U16 => "u16",
        TypeKind::U32 => "u32",
        TypeKind::U64 => "u64",
        TypeKind::F32 => "f32",
        TypeKind::F64 => "f64",
        TypeKind::Char => "char",
        TypeKind::String => "string",
        TypeKind::Bytes => "bytes",
        TypeKind::Array => "array",
        TypeKind::Map => "map",
        TypeKind::Set => "set",
        TypeKind::Range => "range",
        TypeKind::Function => "function",
        TypeKind::Closure => "closure",
        TypeKind::Host => "host",
        TypeKind::ScriptStruct => "struct",
        TypeKind::ScriptEnum => "enum",
    }
}

const fn method_effect_bits(effects: &vela_reflect::access::MethodEffectSet) -> u16 {
    effects.reads_host as u16
        | (effects.writes_host as u16) << 1
        | (effects.emits_events as u16) << 2
        | (effects.reads_time as u16) << 3
        | (effects.uses_random as u16) << 4
        | (effects.reads_io as u16) << 5
        | (effects.writes_io as u16) << 6
        | (effects.reads_reflection as u16) << 7
        | (effects.writes_reflection as u16) << 8
        | (effects.calls_reflection as u16) << 9
}
