use vela_common::{CollectionViewMutation, PrimitiveTag};
use vela_reflect::registry::{TypeDesc, TypeKind, TypeRegistry};
use vela_registry::TypeHintDef;

use crate::type_fact::TypeFact;

pub(super) fn type_desc_fact(registry: &TypeRegistry, desc: &TypeDesc) -> TypeFact {
    if let Some(tag) = PrimitiveTag::from_name(&desc.key.name) {
        return TypeFact::primitive(tag);
    }

    match desc.kind {
        TypeKind::Unit => TypeFact::UNIT,
        TypeKind::Bool => TypeFact::BOOL,
        TypeKind::I8 => TypeFact::I8,
        TypeKind::I16 => TypeFact::I16,
        TypeKind::I32 => TypeFact::I32,
        TypeKind::I64 => TypeFact::I64,
        TypeKind::U8 => TypeFact::U8,
        TypeKind::U16 => TypeFact::U16,
        TypeKind::U32 => TypeFact::U32,
        TypeKind::U64 => TypeFact::U64,
        TypeKind::F32 => TypeFact::F32,
        TypeKind::F64 => TypeFact::F64,
        TypeKind::Char => TypeFact::CHAR,
        TypeKind::String => TypeFact::STRING,
        TypeKind::Bytes => TypeFact::BYTES,
        TypeKind::Tuple => TypeFact::tuple(
            desc.tuple_elements
                .iter()
                .map(|element| registry_hint_fact(registry, element)),
        ),
        TypeKind::Array => TypeFact::array(TypeFact::Any),
        TypeKind::Map => TypeFact::map(TypeFact::Any, TypeFact::Any),
        TypeKind::Set => TypeFact::set(TypeFact::Any),
        TypeKind::Range => TypeFact::Range,
        TypeKind::Function => TypeFact::function(Vec::new(), TypeFact::Any),
        TypeKind::Closure => TypeFact::Closure,
        TypeKind::Host => TypeFact::host(&desc.key.name),
        TypeKind::ScriptStruct => TypeFact::record(&desc.key.name),
        TypeKind::ScriptEnum => TypeFact::enum_type(&desc.key.name, None::<String>),
    }
}

pub(super) fn registry_hint_fact(registry: &TypeRegistry, hint: &str) -> TypeFact {
    TypeHintDef::parse(hint).map_or_else(
        || raw_registry_hint_fact(registry, hint),
        |hint| type_hint_def_fact(registry, &hint),
    )
}

fn type_hint_def_fact(registry: &TypeRegistry, hint: &TypeHintDef) -> TypeFact {
    let path = hint.path.join("::");
    match (path.as_str(), hint.args.as_slice()) {
        ("()", []) => TypeFact::UNIT,
        ("()", elements) if elements.len() >= 2 => TypeFact::tuple(
            elements
                .iter()
                .map(|element| type_hint_def_fact(registry, element)),
        ),
        ("Any", []) => TypeFact::Any,
        ("String", []) => TypeFact::STRING,
        ("Bytes", []) => TypeFact::BYTES,
        ("Array", []) => TypeFact::array(TypeFact::Unknown),
        ("Array", [element]) => TypeFact::array(type_hint_def_fact(registry, element)),
        ("ArrayView", [element]) => TypeFact::array_view(type_hint_def_fact(registry, element)),
        ("ArrayMut", [element]) => TypeFact::array_mut(
            type_hint_def_fact(registry, element),
            hint.collection_mutation
                .unwrap_or(CollectionViewMutation::Fixed),
        ),
        ("Map", []) => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        ("Map", [key, value]) => TypeFact::map(
            type_hint_def_fact(registry, key),
            type_hint_def_fact(registry, value),
        ),
        ("MapView", [key, value]) => TypeFact::map_view(
            type_hint_def_fact(registry, key),
            type_hint_def_fact(registry, value),
        ),
        ("MapMut", [key, value]) => TypeFact::map_mut(
            type_hint_def_fact(registry, key),
            type_hint_def_fact(registry, value),
            hint.collection_mutation
                .unwrap_or(CollectionViewMutation::Growable),
        ),
        ("Set", []) => TypeFact::set(TypeFact::Unknown),
        ("Set", [element]) => TypeFact::set(type_hint_def_fact(registry, element)),
        ("SetView", [element]) => TypeFact::set_view(type_hint_def_fact(registry, element)),
        ("SetMut", [element]) => TypeFact::set_mut(
            type_hint_def_fact(registry, element),
            hint.collection_mutation
                .unwrap_or(CollectionViewMutation::Growable),
        ),
        ("Iterator", []) => TypeFact::iterator(TypeFact::Unknown),
        ("Iterator", [item]) => TypeFact::iterator(type_hint_def_fact(registry, item)),
        ("Function", []) => TypeFact::function(Vec::new(), TypeFact::Unknown),
        ("Closure", []) => TypeFact::Closure,
        ("Option", []) => TypeFact::option(TypeFact::Unknown),
        ("Option", [some]) => TypeFact::option(type_hint_def_fact(registry, some)),
        ("Result", []) => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        ("Result", [ok, err]) => TypeFact::result(
            type_hint_def_fact(registry, ok),
            type_hint_def_fact(registry, err),
        ),
        (name, []) => raw_registry_hint_fact(registry, name),
        _ => TypeFact::Unknown,
    }
}

fn raw_registry_hint_fact(registry: &TypeRegistry, hint: &str) -> TypeFact {
    if let Some(tag) = PrimitiveTag::from_name(hint) {
        return TypeFact::primitive(tag);
    }

    match hint {
        "Any" => TypeFact::Any,
        "String" => TypeFact::STRING,
        "Bytes" => TypeFact::BYTES,
        "Array" => TypeFact::array(TypeFact::Unknown),
        "ArrayView" => TypeFact::array_view(TypeFact::Unknown),
        "ArrayMut" => TypeFact::array_mut(TypeFact::Unknown, CollectionViewMutation::Fixed),
        "Map" => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        "MapView" => TypeFact::map_view(TypeFact::Unknown, TypeFact::Unknown),
        "MapMut" => TypeFact::map_mut(
            TypeFact::Unknown,
            TypeFact::Unknown,
            CollectionViewMutation::Growable,
        ),
        "Set" => TypeFact::set(TypeFact::Unknown),
        "SetView" => TypeFact::set_view(TypeFact::Unknown),
        "SetMut" => TypeFact::set_mut(TypeFact::Unknown, CollectionViewMutation::Growable),
        "Iterator" => TypeFact::iterator(TypeFact::Unknown),
        "Function" => TypeFact::function(Vec::new(), TypeFact::Unknown),
        "Closure" => TypeFact::Closure,
        "Option" => TypeFact::option(TypeFact::Unknown),
        "Result" => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        name => registry.type_by_name(name).map_or_else(
            || trait_or_unknown(registry, name),
            |desc| type_desc_fact(registry, desc),
        ),
    }
}

fn trait_or_unknown(registry: &TypeRegistry, name: &str) -> TypeFact {
    if registry.trait_by_name(name).is_some()
        || registry
            .types()
            .flat_map(|type_desc| type_desc.traits.iter())
            .any(|trait_desc| trait_desc.name == name)
    {
        TypeFact::trait_type(name)
    } else {
        TypeFact::Unknown
    }
}
