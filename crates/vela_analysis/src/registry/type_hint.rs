use super::RegistryFacts;
use crate::type_fact::TypeFact;
use vela_common::{CollectionViewMutation, PrimitiveTag};
use vela_registry::TypeHintDef;

impl RegistryFacts {
    /// Resolve copied registration hints against exact schema type and trait names.
    pub fn type_hint_fact(&self, hint: &TypeHintDef) -> TypeFact {
        type_hint_fact(hint, &|name| {
            self.type_fact(name)
                .or_else(|| self.trait_fact(name))
                .cloned()
        })
    }
}

pub(super) fn type_hint_fact(
    hint: &TypeHintDef,
    resolve: &impl Fn(&str) -> Option<TypeFact>,
) -> TypeFact {
    let name = hint.path.join("::");
    match (name.as_str(), hint.args.as_slice()) {
        ("()", []) => TypeFact::UNIT,
        ("()", elements) if elements.len() >= 2 => TypeFact::tuple(
            elements
                .iter()
                .map(|element| type_hint_fact(element, resolve)),
        ),
        ("Any", []) => TypeFact::Any,
        ("String", []) => TypeFact::STRING,
        ("Bytes", []) => TypeFact::BYTES,
        ("Array", []) => TypeFact::array(TypeFact::Unknown),
        ("Array", [element]) => TypeFact::array(type_hint_fact(element, resolve)),
        ("ArrayView", [element]) => TypeFact::array_view(type_hint_fact(element, resolve)),
        ("ArrayMut", [element]) => TypeFact::array_mut(
            type_hint_fact(element, resolve),
            hint.collection_mutation
                .unwrap_or(CollectionViewMutation::Fixed),
        ),
        ("Map", []) => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        ("Map", [key, value]) => {
            TypeFact::map(type_hint_fact(key, resolve), type_hint_fact(value, resolve))
        }
        ("MapView", [key, value]) => {
            TypeFact::map_view(type_hint_fact(key, resolve), type_hint_fact(value, resolve))
        }
        ("MapMut", [key, value]) => TypeFact::map_mut(
            type_hint_fact(key, resolve),
            type_hint_fact(value, resolve),
            hint.collection_mutation
                .unwrap_or(CollectionViewMutation::Growable),
        ),
        ("Set", []) => TypeFact::set(TypeFact::Unknown),
        ("Set", [element]) => TypeFact::set(type_hint_fact(element, resolve)),
        ("SetView", [element]) => TypeFact::set_view(type_hint_fact(element, resolve)),
        ("SetMut", [element]) => TypeFact::set_mut(
            type_hint_fact(element, resolve),
            hint.collection_mutation
                .unwrap_or(CollectionViewMutation::Growable),
        ),
        ("Iterator", []) => TypeFact::iterator(TypeFact::Unknown),
        ("Iterator", [item]) => TypeFact::iterator(type_hint_fact(item, resolve)),
        ("Function", []) => TypeFact::function(Vec::new(), TypeFact::Unknown),
        ("Closure", []) => TypeFact::Closure,
        ("Option", []) => TypeFact::option(TypeFact::Unknown),
        ("Option", [some]) => TypeFact::option(type_hint_fact(some, resolve)),
        ("Result", []) => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        ("Result", [ok, err]) => {
            TypeFact::result(type_hint_fact(ok, resolve), type_hint_fact(err, resolve))
        }
        (name, []) => PrimitiveTag::from_name(name)
            .map(TypeFact::primitive)
            .or_else(|| resolve(name))
            .unwrap_or(TypeFact::Unknown),
        _ => TypeFact::Unknown,
    }
}
