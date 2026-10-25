//! Owned Rust value registration and its sealed Engine snapshot.

use std::any::{Any, TypeId as RustTypeId};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;

use vela_common::{
    InteropBindingContract, InteropTypeId, TypeAbiFingerprint, TypeBindingRegistryChecksum,
    stable_id,
};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};
use vela_reflect::type_binding::TypeBindingDesc;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::error::{EngineError, EngineErrorKind, EngineResult};

#[derive(Clone)]
pub struct TypeBinding<T: 'static> {
    type_desc: TypeDesc,
    value_codec: ValueCodec<T>,
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
    pub const fn value_with_codec(type_desc: TypeDesc, value_codec: ValueCodec<T>) -> Self {
        Self {
            type_desc,
            value_codec,
        }
    }

    #[must_use]
    pub const fn type_desc(&self) -> &TypeDesc {
        &self.type_desc
    }

    #[must_use]
    pub fn abi_fingerprint(&self) -> TypeAbiFingerprint {
        type_abi_fingerprint(&self.type_desc)
    }

    #[must_use]
    pub fn interop_contract(&self) -> InteropBindingContract {
        InteropBindingContract::new(
            InteropTypeId::from_type_id(self.type_desc.key.id),
            self.abi_fingerprint(),
        )
    }

    pub(crate) fn into_parts(self) -> (TypeBindingRegistration, TypeDesc) {
        let registration = TypeBindingRegistration {
            rust_type_id: None,
            key: self.type_desc.key.clone(),
            value_codec: ErasedValueCodec::new(self.value_codec),
        };
        (registration, self.type_desc)
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
impl<T: IntoScriptArg + FromScriptArg> ValueCodec<T> {
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
    value_codec: ErasedValueCodec,
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
                .expect("registered value descriptor survived validation");
            let id = InteropTypeId::from_type_id(registration.key.id);
            let binding = TypeBindingDesc::new(id, registration.key, type_abi_fingerprint(desc));
            if by_id.insert(id, binding).is_some() {
                return Err(EngineError::new(EngineErrorKind::DuplicateInteropTypeId {
                    id: id.get(),
                }));
            }
            if let Some(rust_type_id) = registration.rust_type_id {
                if let Some(existing) = by_rust_type.insert(rust_type_id, id) {
                    return Err(EngineError::new(
                        EngineErrorKind::DuplicateRustTypeBinding {
                            first: existing.get(),
                            second: id.get(),
                        },
                    ));
                }
                value_codecs_by_rust_type.insert(rust_type_id, registration.value_codec);
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
    pub fn matches_contract(&self, contract: InteropBindingContract) -> bool {
        self.get(contract.type_id)
            .is_some_and(|binding| binding.abi_fingerprint == contract.abi_fingerprint)
    }
    #[must_use]
    pub fn get_for<T: 'static>(&self) -> Option<&TypeBindingDesc> {
        self.by_id
            .get(self.by_rust_type.get(&RustTypeId::of::<T>())?)
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

fn type_abi_fingerprint(desc: &TypeDesc) -> TypeAbiFingerprint {
    let mut parts = vec![
        format!("id={:032x}", desc.key.id.get()),
        format!("path={}", desc.key.name),
        format!("kind={}", type_kind_name(desc.kind)),
        format!(
            "schema={:016x}",
            desc.schema_hash
                .map_or(0, vela_reflect::registry::SchemaHash::get)
        ),
    ];
    let mut fields = desc.fields.iter().collect::<Vec<_>>();
    fields.sort_by_key(|field| field.id);
    parts.extend(fields.into_iter().map(field_abi));
    parts.extend(
        desc.tuple_elements
            .iter()
            .enumerate()
            .map(|(index, element)| format!("tuple-element={index}:{element}")),
    );
    let mut methods = desc.methods.iter().collect::<Vec<_>>();
    methods.sort_by_key(|method| method.id);
    for method in methods {
        parts.push(format!(
            "method={:032x}:{}:{}:{}:{}",
            method.id.get(),
            method.name,
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
                "trait-method={:032x}:{}:{}:{}",
                method.id.get(),
                method.name,
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
    TypeAbiFingerprint::new(stable_id("vela_type_binding_abi_v1", "", &parts.join("|")))
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
        TypeKind::Tuple => "tuple",
        TypeKind::Array => "array",
        TypeKind::Map => "map",
        TypeKind::Set => "set",
        TypeKind::Range => "range",
        TypeKind::Function => "function",
        TypeKind::Closure => "closure",
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
