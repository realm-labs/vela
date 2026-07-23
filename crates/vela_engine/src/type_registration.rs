//! Recursive registration of concrete Rust values used at the Vela boundary.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::builder::EngineBuilder;
use crate::interop::{VelaValueBoundary, VelaValueKeyBoundary};
use crate::standard::{StandardTypeBinding, standard_type_binding};

/// Supplies one complete, concrete owned-Value registration closure.
///
/// Implementations register dependencies before the root. Vela does not gain
/// generic types: `Vec<Item>` and `Vec<Other>` still become separate concrete
/// bindings. `#[derive(Value)]` implements this trait for user DTOs, while this
/// module implements it for supported standard Rust values.
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
    builder.register_generated_rust_value::<T>(standard_type_binding::<T>())
}

macro_rules! leaf_value_types {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RustValueType for $ty {
                fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
                    register_standard::<Self>(builder)
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

impl<T, const N: usize> RustValueType for [T; N]
where
    T: RustValueType,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
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

impl<T> RustValueType for BTreeSet<T>
where
    T: RustValueType + VelaValueKeyBoundary + Ord,
{
    fn register_value_type_closure(builder: EngineBuilder) -> EngineBuilder {
        let builder = T::register_value_type_closure(builder);
        register_standard::<Self>(builder)
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
            .register_rust_type::<i64>(standard_type_binding::<i64>())
            .register_rust_value_closure::<NestedValue>()
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
            .register_rust_type::<i64>(conflicting)
            .register_rust_value_closure::<i64>()
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
            .register_rust_type::<i64>(conflicting)
            .register_rust_value_closure::<i64>()
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
