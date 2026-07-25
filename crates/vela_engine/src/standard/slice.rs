use std::marker::PhantomData;

use vela_common::{
    CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation, HostTypeId,
};
use vela_reflect::registry::{HostIndexCapability, TraitDesc, TypeDesc, TypeKind};

use crate::interop::{VelaHostBoundary, VelaValueBoundary};
use crate::metadata::type_hint_display;
use crate::type_binding::TypeBinding;

use super::bindings::concrete_type_desc;

/// Sized registration key for Rust's dynamically-sized `[T]`.
///
/// The marker is never constructed or exposed to scripts; it lets the unified
/// binding registry describe a concrete slice element type without requiring
/// `TypeBinding` itself to accept dynamically-sized Rust types.
#[doc(hidden)]
pub struct SliceBinding<T>(PhantomData<fn() -> T>);

/// Sized registration key for a borrowed slice whose elements are host objects.
#[doc(hidden)]
pub struct HostSliceBinding<T>(PhantomData<fn() -> T>);

#[must_use]
pub fn standard_slice_type_binding<T>() -> TypeBinding<SliceBinding<T>>
where
    T: VelaValueBoundary + 'static,
{
    TypeBinding::host(slice_type_desc::<T>()).collection_view_capabilities(
        CollectionViewCapabilities::mutable(
            CollectionViewKind::Array,
            CollectionViewMutation::Fixed,
        ),
    )
}

#[doc(hidden)]
#[must_use]
pub fn host_slice_type_binding<T>() -> TypeBinding<HostSliceBinding<T>>
where
    T: VelaHostBoundary + 'static,
{
    TypeBinding::host(host_slice_type_desc::<T>()).collection_view_capabilities(
        CollectionViewCapabilities::mutable(
            CollectionViewKind::Array,
            CollectionViewMutation::Fixed,
        ),
    )
}

#[doc(hidden)]
#[must_use]
pub fn standard_slice_host_type_id<T>() -> HostTypeId
where
    T: VelaValueBoundary + 'static,
{
    let binding = standard_slice_type_binding::<T>();
    HostTypeId::new(
        u64::try_from(binding.type_desc().key.id.get())
            .expect("standard slice type IDs are generated from 64-bit stable IDs"),
    )
}

#[doc(hidden)]
#[must_use]
pub fn host_slice_host_type_id<T>() -> HostTypeId
where
    T: VelaHostBoundary + 'static,
{
    let binding = host_slice_type_binding::<T>();
    HostTypeId::new(
        u64::try_from(binding.type_desc().key.id.get())
            .expect("host slice type IDs are generated from 64-bit stable IDs"),
    )
}

fn slice_type_desc<T>() -> TypeDesc
where
    T: VelaValueBoundary,
{
    let element = type_hint_display(&T::vela_type_hint());
    let desc = concrete_type_desc(
        "slice",
        format!("rust::slice::Slice<{element}>"),
        &element,
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
        "Concrete Rust [{element}] borrowed binding using fixed-length Vela Array behavior."
    ))
    .attr("rust_standard_family", "slice")
    .attr("vela_collection_protocol", "Sequence,Iterable")
    .attr("vela_collection_element", element)
    .attr("vela_collection_growth", "fixed");
    let host_type_id = HostTypeId::new(
        u64::try_from(desc.key.id.get())
            .expect("standard slice type IDs are generated from 64-bit stable IDs"),
    );
    desc.host_type(host_type_id)
}

fn host_slice_type_desc<T>() -> TypeDesc
where
    T: VelaHostBoundary,
{
    let element = type_hint_display(&T::vela_host_type_hint());
    let desc = concrete_type_desc(
        "host_slice",
        format!("rust::slice::HostSlice<{element}>"),
        &element,
        TypeKind::Array,
    )
    .trait_impl(TraitDesc::new("Sequence"))
    .trait_impl(TraitDesc::new("Iterable"))
    .index_capability(
        HostIndexCapability::new()
            .readable(true)
            .writable(false)
            .key_type("i64")
            .value_type(element.clone()),
    )
    .docs(format!(
        "Concrete Rust [{element}] borrowed host-object view with fixed-length Vela Array behavior."
    ))
    .attr("rust_standard_family", "host_slice")
    .attr("vela_collection_protocol", "Sequence,Iterable")
    .attr("vela_collection_element", element)
    .attr("vela_collection_growth", "fixed");
    let host_type_id = HostTypeId::new(
        u64::try_from(desc.key.id.get())
            .expect("host slice type IDs are generated from 64-bit stable IDs"),
    );
    desc.host_type(host_type_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_binding_is_fixed_and_distinct_from_vec_and_array() {
        let slice = standard_slice_type_binding::<i64>();
        assert_eq!(slice.type_desc().kind, TypeKind::Array);
        assert_eq!(
            slice.type_desc().attrs.get("vela_collection_growth"),
            Some("fixed")
        );
        assert_eq!(
            slice.collection_views(),
            Some(CollectionViewCapabilities::mutable(
                CollectionViewKind::Array,
                CollectionViewMutation::Fixed,
            ))
        );
        assert_ne!(
            slice.type_desc().key,
            crate::standard::standard_type_binding::<Vec<i64>>()
                .type_desc()
                .key
        );
        assert_ne!(
            slice.type_desc().key,
            crate::standard::standard_type_binding::<[i64; 3]>()
                .type_desc()
                .key
        );
    }

    #[test]
    fn byte_slice_binding_advertises_fixed_array_views() {
        let slice = standard_slice_type_binding::<u8>();
        assert_eq!(slice.type_desc().kind, TypeKind::Array);
        assert_eq!(
            slice.collection_views(),
            Some(CollectionViewCapabilities::mutable(
                CollectionViewKind::Array,
                CollectionViewMutation::Fixed,
            ))
        );
        assert!(
            slice
                .interop_contract(vela_common::InteropRepresentation::CollectionView(
                    CollectionViewKind::Array,
                ))
                .is_some()
        );
    }
}
