//! Generated building blocks for Rust standard-library type bindings.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use vela_def::TypeId;
use vela_reflect::registry::{
    HostIndexCapability, SchemaHash, TraitDesc, TypeDesc, TypeKey, TypeKind,
};

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::interop::VelaValueBoundary;
use crate::metadata::type_hint_display;
use crate::type_binding::TypeBinding;

/// Supplies the concrete binding used when a generated service type closure
/// reaches a supported Rust standard-library type.
///
/// This is an implementation contract for generated registration bundles. It
/// does not add generic types to Vela: the generated bundle registers only the
/// concrete Rust instantiations present in its service signatures.
pub trait StandardTypeBinding: Sized + 'static {
    fn standard_type_binding() -> TypeBinding<Self>;
}

/// Returns the unified binding for one supported concrete standard Rust type.
///
/// Service macros use this helper while walking their transitive signature
/// graph, so application authors do not register collection instantiations by
/// hand.
#[must_use]
pub fn standard_type_binding<T>() -> TypeBinding<T>
where
    T: StandardTypeBinding,
{
    T::standard_type_binding()
}

impl<K, V> StandardTypeBinding for BTreeMap<K, V>
where
    K: VelaValueBoundary + IntoScriptArg + FromScriptArg + Ord + 'static,
    V: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(map_type_desc::<K, V>(MapFamily::BTree))
    }
}

impl<K, V> StandardTypeBinding for HashMap<K, V>
where
    K: VelaValueBoundary + IntoScriptArg + FromScriptArg + Eq + Hash + 'static,
    V: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(map_type_desc::<K, V>(MapFamily::Hash))
    }
}

#[derive(Clone, Copy)]
enum MapFamily {
    BTree,
    Hash,
}

impl MapFamily {
    const fn rust_name(self) -> &'static str {
        match self {
            Self::BTree => "BTreeMap",
            Self::Hash => "HashMap",
        }
    }

    const fn abi_name(self) -> &'static str {
        match self {
            Self::BTree => "btree_map",
            Self::Hash => "hash_map",
        }
    }
}

fn map_type_desc<K, V>(family: MapFamily) -> TypeDesc
where
    K: VelaValueBoundary,
    V: VelaValueBoundary,
{
    let key = type_hint_display(&K::vela_type_hint());
    let value = type_hint_display(&V::vela_type_hint());
    let path = format!(
        "rust::std::collections::{}<{key}, {value}>",
        family.rust_name()
    );
    let type_id = TypeId::new(u128::from(vela_common::stable_id(
        "rust_standard_type",
        family.abi_name(),
        &format!("{key}|{value}"),
    )));

    TypeDesc::new(TypeKey::new(type_id, path.clone()))
        .kind(TypeKind::Map)
        .schema_hash(SchemaHash::new(vela_common::stable_id(
            "rust_standard_schema",
            family.abi_name(),
            &format!("{key}|{value}"),
        )))
        .trait_impl(TraitDesc::new("MapLike"))
        .index_capability(
            HostIndexCapability::new()
                .readable(true)
                .writable(true)
                .addable(true)
                .removable(true)
                .key_type(key.clone())
                .value_type(value.clone()),
        )
        .docs(format!(
            "Concrete Rust {}<{key}, {value}> value binding using Vela Map behavior.",
            family.rust_name()
        ))
        .attr("rust_standard_family", family.abi_name())
        .attr("vela_collection_protocol", "MapLike")
        .attr("vela_collection_key", key)
        .attr("vela_collection_value", value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{ScalarValue, SourceId};
    use vela_vm::owned_value::OwnedValue;

    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions, Runtime};

    #[test]
    fn concrete_map_families_share_surface_but_keep_stable_identity() {
        let btree = map_type_desc::<String, i64>(MapFamily::BTree);
        let hash = map_type_desc::<String, i64>(MapFamily::Hash);

        assert_ne!(btree.key, hash.key);
        assert_eq!(btree.kind, TypeKind::Map);
        assert_eq!(hash.kind, TypeKind::Map);
        assert_eq!(btree.traits, hash.traits);
        assert_eq!(btree.index_capability, hash.index_capability);
        assert_eq!(btree.attrs.get("vela_collection_protocol"), Some("MapLike"));
        assert_eq!(hash.attrs.get("vela_collection_protocol"), Some("MapLike"));
    }

    #[test]
    fn nested_value_facts_specialize_concrete_map_identity() {
        let strings = map_type_desc::<String, i64>(MapFamily::BTree);
        let integers = map_type_desc::<i64, i64>(MapFamily::BTree);

        assert_ne!(strings.key, integers.key);
        assert_eq!(
            strings
                .index_capability
                .as_ref()
                .and_then(|index| index.key_type.as_deref()),
            Some("String")
        );
        assert_eq!(
            integers
                .index_capability
                .as_ref()
                .and_then(|index| index.key_type.as_deref()),
            Some("i64")
        );
    }

    #[test]
    fn registered_rust_maps_share_vela_map_behavior_and_keep_distinct_abi() {
        type OrderedScores = BTreeMap<String, i64>;
        type HashedScores = HashMap<String, i64>;

        let engine = Engine::builder()
            .with_standard_natives()
            .register_rust_type::<OrderedScores>(standard_type_binding::<OrderedScores>())
            .register_rust_type::<HashedScores>(standard_type_binding::<HashedScores>())
            .build()
            .expect("standard map bindings should seal together");
        let bindings = engine.type_bindings();
        let ordered = bindings
            .get_for::<OrderedScores>()
            .expect("ordered map binding");
        let hashed = bindings
            .get_for::<HashedScores>()
            .expect("hashed map binding");
        assert_ne!(ordered.id, hashed.id);
        assert_ne!(ordered.abi_fingerprint, hashed.abi_fingerprint);

        let registry = engine.registry();
        let ordered_desc = registry
            .type_by_name(&ordered.key.name)
            .expect("ordered map descriptor");
        let hashed_desc = registry
            .type_by_name(&hashed.key.name)
            .expect("hashed map descriptor");
        assert_eq!(ordered_desc.traits, hashed_desc.traits);
        assert_eq!(
            ordered_desc.attrs.get("vela_collection_protocol"),
            Some("MapLike")
        );

        let program = engine
            .compile_source_with_id(
                SourceId::new(1),
                r#"
fn retained_total(scores: Map<String, i64>) -> i64 {
    return scores
        .filter(|value| value >= 5)
        .values()
        .collect_array()
        .sum();
}
"#,
            )
            .expect("shared Vela MapLike behavior should compile");
        let ordered_codec = bindings
            .value_codec::<OrderedScores>()
            .expect("ordered map codec");
        let hashed_codec = bindings
            .value_codec::<HashedScores>()
            .expect("hashed map codec");
        drop(bindings);
        let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

        let ordered_output = runtime
            .call(
                "retained_total",
                CallArgs::from_positional([ordered_codec.encode(BTreeMap::from([
                    ("daily".to_owned(), 2),
                    ("raid".to_owned(), 7),
                ]))]),
                CallOptions::unbounded(),
            )
            .expect("ordered map should use MapLike behavior");
        let hashed_output = runtime
            .call(
                "retained_total",
                CallArgs::from_positional([hashed_codec.encode(HashMap::from([
                    ("daily".to_owned(), 3),
                    ("raid".to_owned(), 8),
                ]))]),
                CallOptions::unbounded(),
            )
            .expect("hashed map should use MapLike behavior");

        assert_eq!(
            runtime.value_to_owned(&ordered_output),
            Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
        );
        assert_eq!(
            runtime.value_to_owned(&hashed_output),
            Ok(OwnedValue::Scalar(ScalarValue::I64(8)))
        );
    }
}
