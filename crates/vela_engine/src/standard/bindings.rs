//! Generated building blocks for Rust standard-library type bindings.

use std::any::TypeId as RustTypeId;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;

use vela_def::TypeId;
use vela_reflect::registry::{
    HostIndexCapability, SchemaHash, TraitDesc, TypeDesc, TypeKey, TypeKind,
};

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::interop::{VelaValueBoundary, VelaValueKeyBoundary};
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

macro_rules! primitive_standard_binding {
    ($($ty:ty => ($family:literal, $path:literal, $kind:ident)),* $(,)?) => {
        $(
            impl StandardTypeBinding for $ty {
                fn standard_type_binding() -> TypeBinding<Self> {
                    TypeBinding::value(primitive_type_desc::<Self>(
                        $family,
                        $path,
                        TypeKind::$kind,
                    ))
                }
            }
        )*
    };
}

primitive_standard_binding!(
    () => ("unit", "rust::primitive::unit", Unit),
    bool => ("bool", "rust::primitive::bool", Bool),
    char => ("char", "rust::primitive::char", Char),
    i8 => ("i8", "rust::primitive::i8", I8),
    i16 => ("i16", "rust::primitive::i16", I16),
    i32 => ("i32", "rust::primitive::i32", I32),
    i64 => ("i64", "rust::primitive::i64", I64),
    u8 => ("u8", "rust::primitive::u8", U8),
    u16 => ("u16", "rust::primitive::u16", U16),
    u32 => ("u32", "rust::primitive::u32", U32),
    u64 => ("u64", "rust::primitive::u64", U64),
    f32 => ("f32", "rust::primitive::f32", F32),
    f64 => ("f64", "rust::primitive::f64", F64),
    String => ("string", "rust::std::string::String", String),
);

impl<T> StandardTypeBinding for Vec<T>
where
    T: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(vec_type_desc::<T>())
    }
}

impl<T> StandardTypeBinding for Option<T>
where
    T: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(option_type_desc::<T>())
    }
}

impl<T, E> StandardTypeBinding for Result<T, E>
where
    T: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    E: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(result_type_desc::<T, E>())
    }
}

impl<A, B> StandardTypeBinding for (A, B)
where
    A: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    B: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(tuple_type_desc::<Self>())
    }
}

impl<A, B, C> StandardTypeBinding for (A, B, C)
where
    A: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    B: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    C: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(tuple_type_desc::<Self>())
    }
}

impl<A, B, C, D> StandardTypeBinding for (A, B, C, D)
where
    A: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    B: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    C: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
    D: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(tuple_type_desc::<Self>())
    }
}

impl<K, V> StandardTypeBinding for BTreeMap<K, V>
where
    K: VelaValueKeyBoundary + IntoScriptArg + FromScriptArg + Ord + 'static,
    V: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(map_type_desc::<K, V>(MapFamily::BTree))
    }
}

impl<K, V> StandardTypeBinding for HashMap<K, V>
where
    K: VelaValueKeyBoundary + IntoScriptArg + FromScriptArg + Eq + Hash + 'static,
    V: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(map_type_desc::<K, V>(MapFamily::Hash))
    }
}

impl<T> StandardTypeBinding for BTreeSet<T>
where
    T: VelaValueKeyBoundary + IntoScriptArg + FromScriptArg + Ord + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(set_type_desc::<T>(SetFamily::BTree))
    }
}

impl<T> StandardTypeBinding for HashSet<T>
where
    T: VelaValueKeyBoundary + IntoScriptArg + FromScriptArg + Eq + Hash + Ord + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(set_type_desc::<T>(SetFamily::Hash))
    }
}

fn primitive_type_desc<T>(family: &'static str, path: &'static str, kind: TypeKind) -> TypeDesc
where
    T: VelaValueBoundary,
{
    let value_shape = type_hint_display(&T::vela_type_hint());
    concrete_type_desc(family, path.to_owned(), &value_shape, kind)
        .docs(format!(
            "Concrete Rust {value_shape} value binding using the Vela {value_shape} representation."
        ))
        .attr("rust_standard_family", family)
        .attr("vela_value_shape", value_shape)
}

fn vec_type_desc<T>() -> TypeDesc
where
    T: VelaValueBoundary + 'static,
{
    if RustTypeId::of::<T>() == RustTypeId::of::<u8>() {
        return concrete_type_desc(
            "vec_bytes",
            "rust::std::vec::Vec<u8>".to_owned(),
            "u8",
            TypeKind::Bytes,
        )
        .trait_impl(TraitDesc::new("Sequence"))
        .trait_impl(TraitDesc::new("Iterable"))
        .docs("Concrete Rust Vec<u8> value binding using Vela Bytes behavior.")
        .attr("rust_standard_family", "vec_bytes")
        .attr("vela_collection_protocol", "Sequence,Iterable")
        .attr("vela_collection_element", "u8")
        .attr("vela_collection_growth", "immutable_bytes");
    }

    let element = type_hint_display(&T::vela_type_hint());
    concrete_type_desc(
        "vec",
        format!("rust::std::vec::Vec<{element}>"),
        &element,
        TypeKind::Array,
    )
    .trait_impl(TraitDesc::new("Sequence"))
    .trait_impl(TraitDesc::new("Iterable"))
    .index_capability(
        HostIndexCapability::new()
            .readable(true)
            .writable(true)
            .addable(true)
            .removable(true)
            .key_type("i64")
            .value_type(element.clone()),
    )
    .docs(format!(
        "Concrete Rust Vec<{element}> value binding using growable Vela Array behavior."
    ))
    .attr("rust_standard_family", "vec")
    .attr("vela_collection_protocol", "Sequence,Iterable")
    .attr("vela_collection_element", element)
    .attr("vela_collection_growth", "growable")
}

fn option_type_desc<T>() -> TypeDesc
where
    T: VelaValueBoundary,
{
    let payload = type_hint_display(&T::vela_type_hint());
    concrete_type_desc(
        "option",
        format!("rust::std::option::Option<{payload}>"),
        &payload,
        TypeKind::ScriptEnum,
    )
    .docs(format!(
        "Concrete Rust Option<{payload}> value binding using Vela Option behavior."
    ))
    .attr("rust_standard_family", "option")
    .attr("vela_value_shape", "Option")
    .attr("vela_option_payload", payload)
}

fn result_type_desc<T, E>() -> TypeDesc
where
    T: VelaValueBoundary,
    E: VelaValueBoundary,
{
    let success = type_hint_display(&T::vela_type_hint());
    let error = type_hint_display(&E::vela_type_hint());
    let facts = format!("{success}|{error}");
    concrete_type_desc(
        "result",
        format!("rust::std::result::Result<{success}, {error}>"),
        &facts,
        TypeKind::ScriptEnum,
    )
    .docs(format!(
        "Concrete Rust Result<{success}, {error}> value binding using Vela Result behavior."
    ))
    .attr("rust_standard_family", "result")
    .attr("vela_value_shape", "Result")
    .attr("vela_result_success", success)
    .attr("vela_result_error", error)
}

fn tuple_type_desc<T>() -> TypeDesc
where
    T: VelaValueBoundary,
{
    let crate::native::TypeHint::TupleOf(elements) = T::vela_type_hint() else {
        unreachable!("tuple binding must expose a tuple type hint")
    };
    let elements = elements.iter().map(type_hint_display).collect::<Vec<_>>();
    let arity = elements.len();
    let facts = format!("({})", elements.join(", "));
    let path = format!("rust::tuple::Tuple{arity}<{}>", elements.join(", "));
    let mut desc = concrete_type_desc("tuple", path, &facts, TypeKind::Tuple)
        .docs(format!(
            "Concrete Rust {arity}-element tuple value binding using the Vela tuple representation."
        ))
        .attr("rust_standard_family", "tuple")
        .attr("vela_value_shape", "Tuple")
        .attr("vela_tuple_arity", arity.to_string())
        .attr("vela_tuple_elements", facts);
    for element in elements {
        desc = desc.tuple_element(element);
    }
    desc
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

#[derive(Clone, Copy)]
enum SetFamily {
    BTree,
    Hash,
}

impl SetFamily {
    const fn rust_name(self) -> &'static str {
        match self {
            Self::BTree => "BTreeSet",
            Self::Hash => "HashSet",
        }
    }

    const fn abi_name(self) -> &'static str {
        match self {
            Self::BTree => "btree_set",
            Self::Hash => "hash_set",
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
    let facts = format!("{key}|{value}");

    concrete_type_desc(family.abi_name(), path, &facts, TypeKind::Map)
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

fn set_type_desc<T>(family: SetFamily) -> TypeDesc
where
    T: VelaValueBoundary,
{
    let element = type_hint_display(&T::vela_type_hint());
    let path = format!("rust::std::collections::{}<{element}>", family.rust_name());

    concrete_type_desc(family.abi_name(), path, &element, TypeKind::Set)
        .trait_impl(TraitDesc::new("SetLike"))
        .trait_impl(TraitDesc::new("Iterable"))
        .docs(format!(
            "Concrete Rust {}<{element}> value binding using Vela Set behavior.",
            family.rust_name()
        ))
        .attr("rust_standard_family", family.abi_name())
        .attr("vela_collection_protocol", "SetLike,Iterable")
        .attr("vela_collection_element", element)
        .attr("vela_collection_growth", "growable")
}

fn concrete_type_desc(family: &str, path: String, facts: &str, kind: TypeKind) -> TypeDesc {
    let type_id = TypeId::new(u128::from(vela_common::stable_id(
        "rust_standard_type",
        family,
        facts,
    )));
    TypeDesc::new(TypeKey::new(type_id, path))
        .kind(kind)
        .schema_hash(SchemaHash::new(vela_common::stable_id(
            "rust_standard_schema",
            family,
            facts,
        )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_analysis::registry::RegistryFacts;
    use vela_analysis::type_fact::TypeFact;
    use vela_common::{ScalarValue, SourceId};
    use vela_vm::owned_value::OwnedValue;

    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions, Runtime};

    #[test]
    fn primitive_bindings_preserve_exact_vela_value_kinds() {
        let bindings = [
            (
                standard_type_binding::<()>().type_desc().kind,
                TypeKind::Unit,
            ),
            (
                standard_type_binding::<bool>().type_desc().kind,
                TypeKind::Bool,
            ),
            (
                standard_type_binding::<char>().type_desc().kind,
                TypeKind::Char,
            ),
            (standard_type_binding::<i8>().type_desc().kind, TypeKind::I8),
            (
                standard_type_binding::<i16>().type_desc().kind,
                TypeKind::I16,
            ),
            (
                standard_type_binding::<i32>().type_desc().kind,
                TypeKind::I32,
            ),
            (
                standard_type_binding::<i64>().type_desc().kind,
                TypeKind::I64,
            ),
            (standard_type_binding::<u8>().type_desc().kind, TypeKind::U8),
            (
                standard_type_binding::<u16>().type_desc().kind,
                TypeKind::U16,
            ),
            (
                standard_type_binding::<u32>().type_desc().kind,
                TypeKind::U32,
            ),
            (
                standard_type_binding::<u64>().type_desc().kind,
                TypeKind::U64,
            ),
            (
                standard_type_binding::<f32>().type_desc().kind,
                TypeKind::F32,
            ),
            (
                standard_type_binding::<f64>().type_desc().kind,
                TypeKind::F64,
            ),
            (
                standard_type_binding::<String>().type_desc().kind,
                TypeKind::String,
            ),
        ];

        assert!(bindings.iter().all(|(actual, expected)| actual == expected));
        assert_ne!(
            standard_type_binding::<i64>().type_desc().key,
            standard_type_binding::<u64>().type_desc().key
        );
    }

    #[test]
    fn registered_rust_primitives_round_trip_through_vela_behavior() {
        let engine = Engine::builder()
            .with_standard_natives()
            .register_rust_type::<i64>(standard_type_binding::<i64>())
            .register_rust_type::<String>(standard_type_binding::<String>())
            .build()
            .expect("primitive bindings should seal beside standard types");
        let bindings = engine.type_bindings();
        let integer_codec = bindings.value_codec::<i64>().expect("i64 value codec");
        let string_codec = bindings
            .value_codec::<String>()
            .expect("String value codec");
        let program = engine
            .compile_source_with_id(
                SourceId::new(4),
                r#"
fn increment(value: i64) -> i64 {
    return value + 1;
}

fn echo_label(value: String) -> String {
    return value;
}
"#,
            )
            .expect("primitive Vela behavior should compile");
        drop(bindings);
        let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

        let integer = runtime
            .call(
                "increment",
                CallArgs::from_positional([integer_codec.encode(8)]),
                CallOptions::unbounded(),
            )
            .expect("registered Rust i64 should use Vela numeric behavior");
        let string = runtime
            .call(
                "echo_label",
                CallArgs::from_positional([string_codec.encode("ready".to_owned())]),
                CallOptions::unbounded(),
            )
            .expect("registered Rust String should use Vela string behavior");

        assert_eq!(
            integer_codec.decode(
                &runtime
                    .value_to_owned(&integer)
                    .expect("returned integer should be owned")
            ),
            Ok(9)
        );
        assert_eq!(
            string_codec.decode(
                &runtime
                    .value_to_owned(&string)
                    .expect("returned string should be owned")
            ),
            Ok("ready".to_owned())
        );
    }

    #[test]
    fn tuple_bindings_preserve_arity_order_and_compiler_facts() {
        type Pair = (i64, String);
        type Reversed = (String, i64);
        type Triple = (i64, String, bool);

        let pair = tuple_type_desc::<Pair>();
        let reversed = tuple_type_desc::<Reversed>();
        let triple = tuple_type_desc::<Triple>();
        assert_eq!(pair.kind, TypeKind::Tuple);
        assert_eq!(pair.tuple_elements, ["i64", "String"]);
        assert_eq!(triple.tuple_elements, ["i64", "String", "bool"]);
        assert_ne!(pair.key, reversed.key);
        assert_ne!(pair.key, triple.key);

        let pair_name = pair.key.name.clone();
        let engine = Engine::builder()
            .register_rust_type::<Pair>(standard_type_binding::<Pair>())
            .build()
            .expect("tuple binding should seal");
        let facts = RegistryFacts::from_compile_view(engine.compiler_registry())
            .expect("tuple compiler facts");
        assert_eq!(
            facts.type_fact(&pair_name),
            Some(&TypeFact::tuple([TypeFact::I64, TypeFact::STRING]))
        );
        let reflected = engine.registry();
        let reflection_facts = RegistryFacts::from_registry(&reflected);
        assert_eq!(
            reflection_facts.type_fact(&pair_name),
            Some(&TypeFact::tuple([TypeFact::I64, TypeFact::STRING]))
        );
    }

    #[test]
    fn registered_rust_tuples_round_trip_through_vela_projections() {
        type Pair = (i64, String);
        type Reversed = (String, i64);

        let engine = Engine::builder()
            .with_standard_natives()
            .register_rust_type::<Pair>(standard_type_binding::<Pair>())
            .register_rust_type::<Reversed>(standard_type_binding::<Reversed>())
            .build()
            .expect("ordered tuple bindings should seal together");
        let bindings = engine.type_bindings();
        let pair_codec = bindings.value_codec::<Pair>().expect("pair codec");
        let reversed_codec = bindings
            .value_codec::<Reversed>()
            .expect("reversed pair codec");
        let program = engine
            .compile_source_with_id(
                SourceId::new(5),
                r#"
fn reverse_and_increment(value: (i64, String)) -> (String, i64) {
    return (value.1, value.0 + 1);
}
"#,
            )
            .expect("tuple projections should compile");
        drop(bindings);
        let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

        let output = runtime
            .call(
                "reverse_and_increment",
                CallArgs::from_positional([pair_codec.encode((8, "ready".to_owned()))]),
                CallOptions::unbounded(),
            )
            .expect("registered Rust tuple should use Vela tuple behavior");
        assert_eq!(
            reversed_codec.decode(
                &runtime
                    .value_to_owned(&output)
                    .expect("returned tuple should be owned")
            ),
            Ok(("ready".to_owned(), 9))
        );
    }

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
    fn vec_and_set_bindings_preserve_representation_capabilities() {
        let vector = vec_type_desc::<i64>();
        let bytes = vec_type_desc::<u8>();
        let ordered = set_type_desc::<i64>(SetFamily::BTree);
        let hashed = set_type_desc::<i64>(SetFamily::Hash);

        assert_eq!(vector.kind, TypeKind::Array);
        assert_eq!(vector.attrs.get("vela_collection_growth"), Some("growable"));
        let index = vector.index_capability.expect("Vec index capability");
        assert!(index.readable && index.writable && index.addable && index.removable);

        assert_eq!(bytes.kind, TypeKind::Bytes);
        assert!(bytes.index_capability.is_none());
        assert_eq!(
            bytes.attrs.get("vela_collection_growth"),
            Some("immutable_bytes")
        );

        assert_ne!(ordered.key, hashed.key);
        assert_eq!(ordered.traits, hashed.traits);
        assert_eq!(
            ordered.attrs.get("vela_collection_protocol"),
            Some("SetLike,Iterable")
        );
    }

    #[test]
    fn option_and_result_bindings_specialize_payload_identity() {
        let integer_option = option_type_desc::<i64>();
        let string_option = option_type_desc::<String>();
        let string_error = result_type_desc::<i64, String>();
        let integer_error = result_type_desc::<i64, i64>();

        assert_ne!(integer_option.key, string_option.key);
        assert_ne!(string_error.key, integer_error.key);
        assert_eq!(integer_option.kind, TypeKind::ScriptEnum);
        assert_eq!(string_error.kind, TypeKind::ScriptEnum);
        assert_eq!(integer_option.attrs.get("vela_value_shape"), Some("Option"));
        assert_eq!(string_error.attrs.get("vela_result_error"), Some("String"));
    }

    #[test]
    fn registered_rust_option_and_result_values_use_vela_behavior() {
        type MaybeScore = Option<i64>;
        type ScoreResult = Result<i64, String>;

        let engine = Engine::builder()
            .with_standard_natives()
            .register_rust_type::<MaybeScore>(standard_type_binding::<MaybeScore>())
            .register_rust_type::<ScoreResult>(standard_type_binding::<ScoreResult>())
            .build()
            .expect("standard Option and Result bindings should seal together");
        let bindings = engine.type_bindings();
        let option_codec = bindings
            .value_codec::<MaybeScore>()
            .expect("Option value codec");
        let result_codec = bindings
            .value_codec::<ScoreResult>()
            .expect("Result value codec");
        assert_ne!(
            bindings.get_for::<MaybeScore>().expect("Option binding").id,
            bindings
                .get_for::<ScoreResult>()
                .expect("Result binding")
                .id
        );

        let program = engine
            .compile_source_with_id(
                SourceId::new(3),
                r#"
fn option_total(value: Option<i64>) -> i64 {
    return value.unwrap_or(0) + 1;
}

fn result_total(value: Result<i64, String>) -> i64 {
    return value.unwrap_or(0) + 2;
}

fn echo_option(value: Option<i64>) -> Option<i64> {
    return value;
}

fn echo_result(value: Result<i64, String>) -> Result<i64, String> {
    return value;
}
"#,
            )
            .expect("shared Vela Option and Result behavior should compile");
        drop(bindings);
        let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

        let some = runtime
            .call(
                "option_total",
                CallArgs::from_positional([option_codec.encode(Some(6))]),
                CallOptions::unbounded(),
            )
            .expect("Rust Some should use Vela Option behavior");
        let none = runtime
            .call(
                "option_total",
                CallArgs::from_positional([option_codec.encode(None)]),
                CallOptions::unbounded(),
            )
            .expect("Rust None should use Vela Option behavior");
        let ok = runtime
            .call(
                "result_total",
                CallArgs::from_positional([result_codec.encode(Ok(7))]),
                CallOptions::unbounded(),
            )
            .expect("Rust Ok should use Vela Result behavior");
        let err = runtime
            .call(
                "result_total",
                CallArgs::from_positional([result_codec.encode(Err("missing".to_owned()))]),
                CallOptions::unbounded(),
            )
            .expect("Rust Err should use Vela Result behavior");
        let returned_option = runtime
            .call(
                "echo_option",
                CallArgs::from_positional([option_codec.encode(Some(11))]),
                CallOptions::unbounded(),
            )
            .expect("Vela should return a registered Rust Option value");
        let returned_result = runtime
            .call(
                "echo_result",
                CallArgs::from_positional([result_codec.encode(Err("retry".to_owned()))]),
                CallOptions::unbounded(),
            )
            .expect("Vela should return a registered Rust Result value");

        assert_eq!(runtime.value_to_owned(&some), Ok(OwnedValue::from(7_i64)));
        assert_eq!(runtime.value_to_owned(&none), Ok(OwnedValue::from(1_i64)));
        assert_eq!(runtime.value_to_owned(&ok), Ok(OwnedValue::from(9_i64)));
        assert_eq!(runtime.value_to_owned(&err), Ok(OwnedValue::from(2_i64)));
        assert_eq!(
            option_codec.decode(
                &runtime
                    .value_to_owned(&returned_option)
                    .expect("returned Option should be owned")
            ),
            Ok(Some(11))
        );
        assert_eq!(
            result_codec.decode(
                &runtime
                    .value_to_owned(&returned_result)
                    .expect("returned Result should be owned")
            ),
            Ok(Err("retry".to_owned()))
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

    #[test]
    fn registered_vec_and_set_values_use_shared_vela_collection_behavior() {
        type Values = Vec<i64>;
        type Bytes = Vec<u8>;
        type Ordered = BTreeSet<i64>;
        type Hashed = HashSet<i64>;

        let engine = Engine::builder()
            .with_standard_natives()
            .register_rust_type::<Values>(standard_type_binding::<Values>())
            .register_rust_type::<Bytes>(standard_type_binding::<Bytes>())
            .register_rust_type::<Ordered>(standard_type_binding::<Ordered>())
            .register_rust_type::<Hashed>(standard_type_binding::<Hashed>())
            .build()
            .expect("standard Vec and Set bindings should seal together");
        let bindings = engine.type_bindings();
        let value_codec = bindings.value_codec::<Values>().expect("Vec codec");
        let byte_codec = bindings.value_codec::<Bytes>().expect("bytes codec");
        let ordered_codec = bindings.value_codec::<Ordered>().expect("BTreeSet codec");
        let hashed_codec = bindings.value_codec::<Hashed>().expect("HashSet codec");
        assert_eq!(
            byte_codec.encode(vec![1, 2, 3]),
            OwnedValue::Bytes(vec![1, 2, 3])
        );
        assert_ne!(
            bindings
                .get_for::<Ordered>()
                .expect("ordered Set binding")
                .id,
            bindings.get_for::<Hashed>().expect("hashed Set binding").id
        );

        let program = engine
            .compile_source_with_id(
                SourceId::new(2),
                r#"
fn array_total(values: Array<i64>) -> i64 {
    return values.filter(|value| value >= 3).sum();
}

fn set_total(values: Set<i64>) -> i64 {
    return values
        .filter(|value| value >= 3)
        .values()
        .collect_array()
        .sum();
}
"#,
            )
            .expect("shared collection protocols should compile");
        drop(bindings);
        let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

        let array_output = runtime
            .call(
                "array_total",
                CallArgs::from_positional([value_codec.encode(vec![1, 3, 5])]),
                CallOptions::unbounded(),
            )
            .expect("Vec should use Sequence behavior");
        let ordered_output = runtime
            .call(
                "set_total",
                CallArgs::from_positional([ordered_codec.encode(BTreeSet::from([1, 3, 5]))]),
                CallOptions::unbounded(),
            )
            .expect("BTreeSet should use SetLike behavior");
        let hashed_output = runtime
            .call(
                "set_total",
                CallArgs::from_positional([hashed_codec.encode(HashSet::from([1, 4, 6]))]),
                CallOptions::unbounded(),
            )
            .expect("HashSet should use SetLike behavior");

        assert_eq!(
            runtime.value_to_owned(&array_output),
            Ok(OwnedValue::from(8_i64))
        );
        assert_eq!(
            runtime.value_to_owned(&ordered_output),
            Ok(OwnedValue::from(8_i64))
        );
        assert_eq!(
            runtime.value_to_owned(&hashed_output),
            Ok(OwnedValue::from(10_i64))
        );
    }
}
