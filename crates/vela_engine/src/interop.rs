//! Language-neutral contracts for explicitly exported Rust and Vela callables.
//!
//! This module describes semantic boundary facts only. Active capability
//! grants, allowlists, reflection permissions, budgets, and other deployment
//! policy deliberately live elsewhere and cannot affect an ABI fingerprint.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use vela_common::{CallableAsyncness, CapabilitySet, Span, stable_id};

use crate::native::{EffectSet, TypeHint};
use crate::schema::ScriptHostSchema;

/// Deterministic metadata proof for an ordinary owned/copy boundary value.
/// Conversion is performed by the existing `IntoScriptArg`/`FromScriptArg`
/// traits; this trait prevents exported signatures from silently degrading to
/// `Any` when no stable type contract exists.
pub trait VelaValueBoundary {
    fn vela_type_hint() -> TypeHint;
}

/// Deterministic metadata proof for an exact registered host type.
pub trait VelaHostBoundary: ScriptHostSchema {
    fn vela_host_type_hint() -> TypeHint {
        TypeHint::Host(Self::script_host_type_desc().key)
    }
}

impl<T: ScriptHostSchema> VelaHostBoundary for T {}

macro_rules! primitive_boundary {
    ($($ty:ty => $hint:ident),* $(,)?) => {
        $(
            impl VelaValueBoundary for $ty {
                fn vela_type_hint() -> TypeHint {
                    TypeHint::$hint()
                }
            }
        )*
    };
}

primitive_boundary!(
    () => unit,
    bool => boolean,
    char => char,
    i8 => i8,
    i16 => i16,
    i32 => i32,
    i64 => i64,
    u8 => u8,
    u16 => u16,
    u32 => u32,
    u64 => u64,
    f32 => f32,
    f64 => f64,
    String => string,
);

impl<T: VelaValueBoundary> VelaValueBoundary for Vec<T> {
    fn vela_type_hint() -> TypeHint {
        TypeHint::array_of(T::vela_type_hint())
    }
}

impl<T: VelaValueBoundary, const N: usize> VelaValueBoundary for [T; N] {
    fn vela_type_hint() -> TypeHint {
        TypeHint::array_of(T::vela_type_hint())
    }
}

impl<T: VelaValueBoundary> VelaValueBoundary for Option<T> {
    fn vela_type_hint() -> TypeHint {
        TypeHint::option_of(T::vela_type_hint())
    }
}

impl<T: VelaValueBoundary, E: VelaValueBoundary> VelaValueBoundary for Result<T, E> {
    fn vela_type_hint() -> TypeHint {
        TypeHint::result_of(T::vela_type_hint(), E::vela_type_hint())
    }
}

impl<K, V> VelaValueBoundary for BTreeMap<K, V>
where
    K: VelaValueBoundary,
    V: VelaValueBoundary,
{
    fn vela_type_hint() -> TypeHint {
        TypeHint::map_of(K::vela_type_hint(), V::vela_type_hint())
    }
}

impl<K, V> VelaValueBoundary for HashMap<K, V>
where
    K: VelaValueBoundary + Eq + Hash,
    V: VelaValueBoundary,
{
    fn vela_type_hint() -> TypeHint {
        TypeHint::map_of(K::vela_type_hint(), V::vela_type_hint())
    }
}

impl<T: VelaValueBoundary> VelaValueBoundary for BTreeSet<T> {
    fn vela_type_hint() -> TypeHint {
        TypeHint::set_of(T::vela_type_hint())
    }
}

impl<T> VelaValueBoundary for HashSet<T>
where
    T: VelaValueBoundary + Eq + Hash,
{
    fn vela_type_hint() -> TypeHint {
        TypeHint::set_of(T::vela_type_hint())
    }
}

/// Stable semantic category of a cross-language callable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableKind {
    RustFunction,
    RustMethod,
    RustTraitMethod,
    VelaFunction,
    VelaMethod,
    VelaProviderMethod,
}

impl CallableKind {
    const fn abi_name(self) -> &'static str {
        match self {
            Self::RustFunction => "rust_function",
            Self::RustMethod => "rust_method",
            Self::RustTraitMethod => "rust_trait_method",
            Self::VelaFunction => "vela_function",
            Self::VelaMethod => "vela_method",
            Self::VelaProviderMethod => "vela_provider_method",
        }
    }
}

/// Semantic callable identity. The stable value is derived at declaration or
/// link time; runtime strings are never the prepared-call locator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableIdentity {
    pub kind: CallableKind,
    pub stable: u128,
}

impl CallableIdentity {
    #[must_use]
    pub const fn new(kind: CallableKind, stable: u128) -> Self {
        Self { kind, stable }
    }
}

/// Stable cross-language parameter mode.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundaryMode {
    Value,
    ReadOnlyValueBorrow,
    SharedHost,
    ExclusiveHost,
    HiddenContext,
}

impl BoundaryMode {
    const fn abi_name(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::ReadOnlyValueBorrow => "readonly_value_borrow",
            Self::SharedHost => "shared_host",
            Self::ExclusiveHost => "exclusive_host",
            Self::HiddenContext => "hidden_context",
        }
    }
}

/// Parent parameter retaining authority for a scoped borrowed host return.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BorrowedReturnOrigin {
    Receiver,
    Parameter(u16),
}

/// Access retained by a call-tree-scoped host return.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScopedHostAccess {
    Shared,
    Exclusive,
}

impl ScopedHostAccess {
    const fn abi_name(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Exclusive => "exclusive",
        }
    }
}

/// Stable return representation at the language boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReturnMode {
    OwnedValue,
    StructuredValue,
    ScopedHost {
        origin: BorrowedReturnOrigin,
        child_access: ScopedHostAccess,
        parent_freeze: ScopedHostAccess,
    },
}

/// Whether a Rust return reports invocation failure or yields an ordinary
/// Vela value. `Result<T, E>` is `Value`; `VmResult<T>` is `RuntimeResult`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorMode {
    Value,
    RuntimeResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameter {
    pub identity: u64,
    pub name: String,
    pub ty: TypeHint,
    pub mode: BoundaryMode,
}

impl CallableParameter {
    #[must_use]
    pub fn new(identity: u64, name: impl Into<String>, ty: TypeHint, mode: BoundaryMode) -> Self {
        Self {
            identity,
            name: name.into(),
            ty,
            mode,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableReturn {
    pub ty: TypeHint,
    pub mode: ReturnMode,
    pub error_mode: ErrorMode,
}

impl CallableReturn {
    #[must_use]
    pub const fn new(ty: TypeHint, mode: ReturnMode, error_mode: ErrorMode) -> Self {
        Self {
            ty,
            mode,
            error_mode,
        }
    }
}

/// Semantic visibility. These flags are ABI; live reflection permissions are
/// deployment policy and are intentionally absent.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CallableAccess {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
}

impl Default for CallableAccess {
    fn default() -> Self {
        Self {
            public: true,
            reflect_visible: true,
            reflect_callable: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableLanguage {
    Rust,
    Vela,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableOrigin {
    pub language: CallableLanguage,
    pub source_span: Option<Span>,
}

/// Deterministic ABI fingerprint for generated bindings and hot reload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CallableAbiFingerprint(u64);

impl CallableAbiFingerprint {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Shared reflection/link/runtime contract for a cross-language callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableContract {
    pub identity: CallableIdentity,
    pub public_path: String,
    pub parameters: Vec<CallableParameter>,
    pub returns: CallableReturn,
    pub asyncness: CallableAsyncness,
    pub effects: EffectSet,
    pub access: CallableAccess,
    pub docs: Option<String>,
    pub origin: CallableOrigin,
}

impl CallableContract {
    /// Computes the required coarse capabilities from the normalized effect
    /// set. This is the sole callable-to-capability projection.
    #[must_use]
    pub const fn required_capabilities(&self) -> CapabilitySet {
        self.effects.required_capability_set()
    }

    /// Computes a deterministic fingerprint from semantic ABI only.
    #[must_use]
    pub fn abi_fingerprint(&self) -> CallableAbiFingerprint {
        let canonical = self.canonical_abi();
        CallableAbiFingerprint::new(stable_id(
            "callable_abi_v1",
            self.identity.kind.abi_name(),
            &canonical,
        ))
    }

    /// Produces field-level, human-readable incompatibilities. Documentation,
    /// source position, and deployment policy are intentionally ignored.
    #[must_use]
    pub fn abi_diff(&self, candidate: &Self) -> Vec<CallableAbiDifference> {
        let mut differences = Vec::new();
        push_difference(
            &mut differences,
            "identity",
            &self.identity,
            &candidate.identity,
        );
        push_difference(
            &mut differences,
            "public_path",
            &self.public_path,
            &candidate.public_path,
        );
        push_difference(
            &mut differences,
            "parameters",
            &self.parameters,
            &candidate.parameters,
        );
        push_difference(
            &mut differences,
            "return",
            &self.returns,
            &candidate.returns,
        );
        push_difference(
            &mut differences,
            "asyncness",
            &self.asyncness,
            &candidate.asyncness,
        );
        push_difference(
            &mut differences,
            "effects",
            &self.effects,
            &candidate.effects,
        );
        push_difference(&mut differences, "access", &self.access, &candidate.access);
        differences
    }

    fn canonical_abi(&self) -> String {
        let mut value = format!(
            "{}|{}|{:032x}|{}|{}|{}|{}|{}",
            self.identity.kind.abi_name(),
            self.public_path,
            self.identity.stable,
            asyncness_name(self.asyncness),
            self.effects.bits(),
            u8::from(self.access.public),
            u8::from(self.access.reflect_visible),
            u8::from(self.access.reflect_callable),
        );
        for parameter in &self.parameters {
            value.push_str(&format!(
                "|p:{:016x}:{}:{}:",
                parameter.identity,
                parameter.name,
                parameter.mode.abi_name()
            ));
            encode_hint(&parameter.ty, &mut value);
        }
        value.push_str("|r:");
        encode_return_mode(self.returns.mode, &mut value);
        value.push(':');
        value.push_str(match self.returns.error_mode {
            ErrorMode::Value => "value",
            ErrorMode::RuntimeResult => "runtime_result",
        });
        value.push(':');
        encode_hint(&self.returns.ty, &mut value);
        value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableAbiDifference {
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for CallableAbiDifference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "callable ABI field `{}` changed: expected {}, got {}",
            self.field, self.expected, self.actual
        )
    }
}

fn push_difference<T: fmt::Debug + PartialEq>(
    differences: &mut Vec<CallableAbiDifference>,
    field: &'static str,
    expected: &T,
    actual: &T,
) {
    if expected != actual {
        differences.push(CallableAbiDifference {
            field,
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
}

fn asyncness_name(asyncness: CallableAsyncness) -> &'static str {
    match asyncness {
        CallableAsyncness::Sync => "sync",
        CallableAsyncness::Async => "async",
    }
}

fn encode_return_mode(mode: ReturnMode, output: &mut String) {
    match mode {
        ReturnMode::OwnedValue => output.push_str("owned"),
        ReturnMode::StructuredValue => output.push_str("structured"),
        ReturnMode::ScopedHost {
            origin,
            child_access,
            parent_freeze,
        } => {
            output.push_str("scoped_host:");
            match origin {
                BorrowedReturnOrigin::Receiver => output.push_str("receiver"),
                BorrowedReturnOrigin::Parameter(index) => {
                    output.push_str(&format!("parameter_{index}"));
                }
            }
            output.push(':');
            output.push_str(child_access.abi_name());
            output.push(':');
            output.push_str(parent_freeze.abi_name());
        }
    }
}

fn encode_hint(hint: &TypeHint, output: &mut String) {
    match hint {
        TypeHint::Any => output.push_str("any"),
        TypeHint::Primitive(tag) => output.push_str(tag.name()),
        TypeHint::Array => output.push_str("array"),
        TypeHint::ArrayOf(element) => encode_unary_hint("array", element, output),
        TypeHint::Map => output.push_str("map"),
        TypeHint::MapOf { key, value } => {
            output.push_str("map<");
            encode_hint(key, output);
            output.push(',');
            encode_hint(value, output);
            output.push('>');
        }
        TypeHint::Set => output.push_str("set"),
        TypeHint::SetOf(element) => encode_unary_hint("set", element, output),
        TypeHint::Iterator => output.push_str("iterator"),
        TypeHint::IteratorOf(element) => encode_unary_hint("iterator", element, output),
        TypeHint::OptionOf(element) => encode_unary_hint("option", element, output),
        TypeHint::ResultOf { ok, err } => {
            output.push_str("result<");
            encode_hint(ok, output);
            output.push(',');
            encode_hint(err, output);
            output.push('>');
        }
        TypeHint::PathProxy => output.push_str("path_proxy"),
        TypeHint::Record(key) => encode_type_key("record", key, output),
        TypeHint::Enum(key) => encode_type_key("enum", key, output),
        TypeHint::Host(key) => encode_type_key("host", key, output),
        TypeHint::Trait(name) => {
            output.push_str("trait:");
            output.push_str(name);
        }
        TypeHint::Function => output.push_str("function"),
    }
}

fn encode_unary_hint(name: &str, element: &TypeHint, output: &mut String) {
    output.push_str(name);
    output.push('<');
    encode_hint(element, output);
    output.push('>');
}

fn encode_type_key(name: &str, key: &vela_reflect::registry::TypeKey, output: &mut String) {
    output.push_str(name);
    output.push(':');
    output.push_str(&format!("{:032x}", key.id.get()));
    output.push(':');
    output.push_str(&key.name);
}

#[cfg(test)]
mod tests {
    use vela_common::{CallableAsyncness, Capability};

    use super::{
        BoundaryMode, CallableAccess, CallableContract, CallableIdentity, CallableKind,
        CallableLanguage, CallableOrigin, CallableParameter, CallableReturn, ErrorMode, ReturnMode,
    };
    use crate::native::{EffectSet, TypeHint};

    fn contract(effects: EffectSet) -> CallableContract {
        CallableContract {
            identity: CallableIdentity::new(CallableKind::RustFunction, 42),
            public_path: "game::grant_exp".to_owned(),
            parameters: vec![CallableParameter::new(
                7,
                "amount",
                TypeHint::i64(),
                BoundaryMode::Value,
            )],
            returns: CallableReturn::new(
                TypeHint::unit(),
                ReturnMode::OwnedValue,
                ErrorMode::RuntimeResult,
            ),
            asyncness: CallableAsyncness::Sync,
            effects,
            access: CallableAccess::default(),
            docs: Some("not ABI".to_owned()),
            origin: CallableOrigin {
                language: CallableLanguage::Rust,
                source_span: None,
            },
        }
    }

    #[test]
    fn callable_fingerprint_is_deterministic_and_ignores_docs() {
        let first = contract(EffectSet::host_write());
        let mut second = first.clone();
        second.docs = Some("new docs".to_owned());

        assert_eq!(first.abi_fingerprint(), second.abi_fingerprint());
        assert!(first.abi_diff(&second).is_empty());
    }

    #[test]
    fn redundant_effect_union_does_not_change_fingerprint() {
        let inferred = contract(EffectSet::host_write());
        let redundant = contract(EffectSet::host_write().union(EffectSet::host_read()));

        assert_eq!(inferred.effects, redundant.effects);
        assert_eq!(inferred.abi_fingerprint(), redundant.abi_fingerprint());
    }

    #[test]
    fn effect_projection_is_canonical_and_excludes_redundant_host_read() {
        let contract = contract(EffectSet::host_write().union(EffectSet::random()));
        let capabilities = contract.required_capabilities();

        assert!(capabilities.contains(Capability::HostWrite));
        assert!(capabilities.contains(Capability::Random));
        assert!(!capabilities.contains(Capability::HostRead));
    }

    #[test]
    fn abi_diff_names_changed_semantic_fields() {
        let expected = contract(EffectSet::host_read());
        let actual = contract(EffectSet::host_write());
        let differences = expected.abi_diff(&actual);

        assert_eq!(differences.len(), 1);
        assert_eq!(differences[0].field, "effects");
        assert!(
            differences[0]
                .to_string()
                .contains("callable ABI field `effects` changed")
        );
    }
}
