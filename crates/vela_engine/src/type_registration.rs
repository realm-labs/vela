//! Recursive registration of concrete Rust values used at the Vela boundary.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::builder::EngineBuilder;
use crate::interop::{VelaValueBoundary, VelaValueKeyBoundary};
use crate::standard::{StandardTypeBinding, standard_type_binding};

/// Registers one complete Rust type surface with an [`EngineBuilder`].
///
/// This is the single user-facing type-registration contract. Implementations
/// install the full transitive dependency closure for structural Values or the
/// exact Host binding for Rust-owned objects.
pub trait VelaType: Sized + 'static {
    /// Returns the complete registration object for this concrete Rust type.
    #[must_use]
    fn vela_type() -> crate::registration::TypeRegistration<Self> {
        crate::registration::TypeRegistration::of()
    }

    #[doc(hidden)]
    fn register(builder: EngineBuilder) -> EngineBuilder;
}

/// Supplies one complete, concrete owned-Value registration closure.
///
/// Implementations register dependencies before the root. Vela does not gain
/// generic types: `Vec<Item>` and `Vec<Other>` still become separate concrete
/// bindings. `#[derive(Value)]` implements this trait for user DTOs, while this
/// module implements it for supported standard Rust values.
#[doc(hidden)]
pub trait RustValueType:
    VelaValueBoundary + IntoScriptArg + FromScriptArg + Sized + 'static
{
    #[doc(hidden)]
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder;
}

fn register_standard<T>(builder: EngineBuilder) -> EngineBuilder
where
    T: RustValueType + StandardTypeBinding,
{
    builder.register_generated_type_binding::<T>(standard_type_binding::<T>())
}

macro_rules! leaf_value_types {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RustValueType for $ty {
                fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
                    register_standard::<Self>(builder)
                }
            }

            impl VelaType for $ty {
                fn register(builder: EngineBuilder) -> EngineBuilder {
                    <Self as RustValueType>::register_value_type_closure(builder)
                }
            }
        )*
    };
}

leaf_value_types!(
    (),
    bool,
    char,
    i8,
    i16,
    i32,
    i64,
    u8,
    u16,
    u32,
    u64,
    f32,
    f64,
    String,
);

impl<T> RustValueType for Vec<T>
where
    T: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<T> VelaType for Vec<T>
where
    T: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<T, const N: usize> RustValueType for [T; N]
where
    T: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<T, const N: usize> VelaType for [T; N]
where
    T: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<T> RustValueType for Option<T>
where
    T: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<T> VelaType for Option<T>
where
    T: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<T, E> RustValueType for Result<T, E>
where
    T: RustValueType,
    E: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        let builder = E::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<T, E> VelaType for Result<T, E>
where
    T: RustValueType,
    E: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<A, B> RustValueType for (A, B)
where
    A: RustValueType,
    B: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = A::register_value_type_closure(builder);
        let builder = B::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<A, B> VelaType for (A, B)
where
    A: RustValueType,
    B: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<A, B, C> RustValueType for (A, B, C)
where
    A: RustValueType,
    B: RustValueType,
    C: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = A::register_value_type_closure(builder);
        let builder = B::register_value_type_closure(builder);
        let builder = C::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<A, B, C> VelaType for (A, B, C)
where
    A: RustValueType,
    B: RustValueType,
    C: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<A, B, C, D> RustValueType for (A, B, C, D)
where
    A: RustValueType,
    B: RustValueType,
    C: RustValueType,
    D: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = A::register_value_type_closure(builder);
        let builder = B::register_value_type_closure(builder);
        let builder = C::register_value_type_closure(builder);
        let builder = D::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<A, B, C, D> VelaType for (A, B, C, D)
where
    A: RustValueType,
    B: RustValueType,
    C: RustValueType,
    D: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<K, V> RustValueType for BTreeMap<K, V>
where
    K: RustValueType + VelaValueKeyBoundary + Ord,
    V: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = K::register_value_type_closure(builder);
        let builder = V::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<K, V> VelaType for BTreeMap<K, V>
where
    K: RustValueType + VelaValueKeyBoundary + Ord,
    V: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<K, V> RustValueType for HashMap<K, V>
where
    K: RustValueType + VelaValueKeyBoundary + Eq + Hash,
    V: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = K::register_value_type_closure(builder);
        let builder = V::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<K, V> VelaType for HashMap<K, V>
where
    K: RustValueType + VelaValueKeyBoundary + Eq + Hash,
    V: RustValueType,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<T> RustValueType for BTreeSet<T>
where
    T: RustValueType + VelaValueKeyBoundary + Ord,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<T> VelaType for BTreeSet<T>
where
    T: RustValueType + VelaValueKeyBoundary + Ord,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

impl<T> RustValueType for HashSet<T>
where
    T: RustValueType + VelaValueKeyBoundary + Eq + Hash + Ord,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
    }
}

impl<T> VelaType for HashSet<T>
where
    T: RustValueType + VelaValueKeyBoundary + Eq + Hash + Ord,
{
    fn register(builder: EngineBuilder) -> EngineBuilder {
        <Self as RustValueType>::register_value_type_closure(builder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::ReceiverCapabilities;
    use vela_def::TypeId;
    use vela_reflect::registry::{TypeDesc, TypeKey, TypeKind};

    use crate::engine::Engine;
    use crate::error::EngineErrorKind;
    use crate::type_binding::TypeBinding;

    type NestedValue = (Vec<i64>, BTreeMap<String, Option<i64>>);

    #[test]
    fn standard_value_closure_registers_nested_types_once() {
        let engine = Engine::builder()
            .install_type_binding::<i64>(standard_type_binding::<i64>())
            .install_generated_type::<NestedValue>()
            .build()
            .expect("exact prior registrations and shared dependencies should deduplicate");
        let bindings = engine.type_bindings();

        assert!(bindings.get_for::<i64>().is_some());
        assert!(bindings.get_for::<String>().is_some());
        assert!(bindings.get_for::<Vec<i64>>().is_some());
        assert!(bindings.get_for::<Option<i64>>().is_some());
        assert!(
            bindings
                .get_for::<BTreeMap<String, Option<i64>>>()
                .is_some()
        );
        assert!(bindings.get_for::<NestedValue>().is_some());
        assert_eq!(bindings.iter().count(), 6);

        let codec = bindings
            .value_codec::<NestedValue>()
            .expect("root closure should install its concrete codec");
        let value = (vec![2, 3], BTreeMap::from([("score".to_owned(), Some(5))]));
        assert_eq!(codec.decode(&codec.encode(value.clone())), Ok(value));
    }

    #[test]
    fn generated_value_closure_does_not_hide_conflicting_manual_binding() {
        let conflicting = TypeBinding::<i64>::value(
            TypeDesc::new(TypeKey::new(TypeId::new(9_001), "host::AlternateI64"))
                .kind(TypeKind::I64),
        );
        let result = Engine::builder()
            .install_type_binding::<i64>(conflicting)
            .install_generated_type::<i64>()
            .build();

        assert!(matches!(
            result,
            Err(error)
                if matches!(error.kind, EngineErrorKind::DuplicateRustTypeBinding { .. })
        ));
    }

    #[test]
    fn generated_value_closure_compares_the_complete_pending_abi() {
        let conflicting =
            standard_type_binding::<i64>().receiver_capabilities(ReceiverCapabilities::OWNED);
        let result = Engine::builder()
            .install_type_binding::<i64>(conflicting)
            .install_generated_type::<i64>()
            .build();

        assert!(matches!(
            result,
            Err(error)
                if matches!(
                    error.kind,
                    EngineErrorKind::DuplicateTypeId { .. }
                        | EngineErrorKind::DuplicateTypeName { .. }
                )
        ));
    }
}
