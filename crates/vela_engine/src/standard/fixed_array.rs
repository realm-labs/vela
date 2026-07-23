use vela_common::{CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation};
use vela_reflect::registry::{HostIndexCapability, TraitDesc, TypeDesc, TypeKind};

use crate::args::{FromScriptArg, IntoScriptArg};
use crate::interop::VelaValueBoundary;
use crate::metadata::type_hint_display;
use crate::type_binding::TypeBinding;

use super::bindings::{StandardTypeBinding, concrete_type_desc};

impl<T, const N: usize> StandardTypeBinding for [T; N]
where
    T: VelaValueBoundary + IntoScriptArg + FromScriptArg + 'static,
{
    fn standard_type_binding() -> TypeBinding<Self> {
        TypeBinding::value(fixed_array_type_desc::<T, N>()).collection_view_capabilities(
            CollectionViewCapabilities::mutable(
                CollectionViewKind::Array,
                CollectionViewMutation::Fixed,
            ),
        )
    }
}

fn fixed_array_type_desc<T, const N: usize>() -> TypeDesc
where
    T: VelaValueBoundary,
{
    let element = type_hint_display(&T::vela_type_hint());
    concrete_type_desc(
        "fixed_array",
        format!("rust::array::Array<{element}, {N}>"),
        &format!("{element};{N}"),
        TypeKind::Array,
    )
    .trait_impl(TraitDesc::new("Sequence"))
    .trait_impl(TraitDesc::new("Iterable"))
    .index_capability(
        HostIndexCapability::new()
            .readable(true)
            .writable(true)
            .key_type("i64")
            .value_type(element.clone()),
    )
    .docs(format!(
        "Concrete Rust [{element}; {N}] value binding using fixed-length Vela Array behavior."
    ))
    .attr("rust_standard_family", "fixed_array")
    .attr("vela_collection_protocol", "Sequence,Iterable")
    .attr("vela_collection_element", element)
    .attr("vela_collection_growth", "fixed")
    .attr("vela_collection_length", N.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard::standard_type_binding;

    #[test]
    fn fixed_array_binding_preserves_length_identity_and_view_capability() {
        let fixed = fixed_array_type_desc::<i64, 3>();
        let other = fixed_array_type_desc::<i64, 4>();
        assert_ne!(fixed.key, other.key);
        assert_eq!(fixed.kind, TypeKind::Array);
        assert_eq!(fixed.attrs.get("vela_collection_growth"), Some("fixed"));
        let index = fixed
            .index_capability
            .expect("fixed array index capability");
        assert!(index.readable && index.writable);
        assert!(!index.addable && !index.removable);
        assert_eq!(
            standard_type_binding::<[i64; 3]>().collection_views(),
            Some(CollectionViewCapabilities::mutable(
                CollectionViewKind::Array,
                CollectionViewMutation::Fixed,
            ))
        );
    }
}
