use proc_macro2::TokenStream;
use quote::quote;

use crate::export::signature::{
    BorrowedCollectionKind, ClassifiedSignature, HostAccess, TypeShape,
};

pub(super) fn binding_use_tokens(contract: TokenStream, shape: &TypeShape) -> TokenStream {
    let TypeShape::BorrowedCollection(collection) = shape else {
        return contract;
    };
    let rust_ty = &collection.rust_ty;
    let kind = match &collection.kind {
        BorrowedCollectionKind::Array(_) => {
            quote! { ::vela_common::CollectionViewKind::Array }
        }
        BorrowedCollectionKind::Map(_, _) => {
            quote! { ::vela_common::CollectionViewKind::Map }
        }
        BorrowedCollectionKind::Set(_) => {
            quote! { ::vela_common::CollectionViewKind::Set }
        }
    };
    let representation = if collection.access == HostAccess::Shared {
        quote! { ::vela_common::InteropRepresentation::CollectionView(#kind) }
    } else {
        let mutation = match collection.mutation {
            vela_common::CollectionViewMutation::Fixed => {
                quote! { ::vela_common::CollectionViewMutation::Fixed }
            }
            vela_common::CollectionViewMutation::Growable => {
                quote! { ::vela_common::CollectionViewMutation::Growable }
            }
        };
        quote! {
            ::vela_common::InteropRepresentation::CollectionMut {
                kind: #kind,
                mutation: #mutation,
            }
        }
    };
    let binding = collection.slice_element.as_ref().map_or_else(
        || quote! { ::vela_engine::standard::standard_type_binding::<#rust_ty>() },
        |element| quote! { ::vela_engine::standard::standard_slice_type_binding::<#element>() },
    );
    quote! {
        (#contract).with_binding(
            #binding
                .interop_contract(#representation)
                .expect("generated borrowed collection representation must be registered"),
        )
    }
}

pub(crate) fn host_type_id_tokens(shape: &TypeShape) -> Option<TokenStream> {
    match shape {
        TypeShape::Host(ty, _) => {
            Some(quote! { <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_id() })
        }
        TypeShape::BorrowedCollection(collection) => {
            Some(collection.slice_element.as_ref().map_or_else(
                || {
                    let ty = &collection.rust_ty;
                    quote! { ::vela_engine::standard::standard_collection_host_type_id::<#ty>() }
                },
                |element| {
                    quote! { ::vela_engine::standard::standard_slice_host_type_id::<#element>() }
                },
            ))
        }
        _ => None,
    }
}

pub(super) fn collection_registration_tokens(signature: &ClassifiedSignature) -> Vec<TokenStream> {
    fn collect(shape: &TypeShape, output: &mut Vec<TokenStream>) {
        match shape {
            TypeShape::BorrowedCollection(collection) => {
                output.push(collection.slice_element.as_ref().map_or_else(
                    || {
                        let ty = &collection.rust_ty;
                        quote! { let builder = builder.register_rust_value_closure::<#ty>(); }
                    },
                    |element| {
                        quote! { let builder = builder.register_rust_slice::<#element>(); }
                    },
                ));
            }
            TypeShape::Option(inner) => collect(inner, output),
            TypeShape::Result(ok, err) => {
                collect(ok, output);
                collect(err, output);
            }
            TypeShape::Tuple(elements) => {
                for element in elements {
                    collect(element, output);
                }
            }
            _ => {}
        }
    }

    let mut registrations = Vec::new();
    for parameter in &signature.parameters {
        collect(&parameter.ty, &mut registrations);
    }
    collect(&signature.returns.ty, &mut registrations);
    registrations
}

pub(crate) fn shared_host_value_tokens(shape: &TypeShape, object: TokenStream) -> TokenStream {
    if let Some(element) = shape.borrowed_slice_element() {
        quote! { ::vela_host::object::lease_slice_ref::<#element>(#object) }
    } else {
        let ty = match shape {
            TypeShape::StorageDirectedShared(ty) => ty,
            _ => {
                let (ty, _) = shape
                    .host_boundary()
                    .expect("shared host extraction requires a host boundary");
                ty
            }
        };
        quote! { (#object).lease_any().and_then(|object| object.downcast_ref::<#ty>()) }
    }
}

pub(crate) fn exclusive_host_value_tokens(shape: &TypeShape, object: TokenStream) -> TokenStream {
    if let Some(element) = shape.borrowed_slice_element() {
        quote! { ::vela_host::object::lease_slice_mut::<#element>(#object) }
    } else {
        let (ty, _) = shape
            .host_boundary()
            .expect("exclusive host extraction requires a host boundary");
        quote! { (#object).lease_any_mut().and_then(|object| object.downcast_mut::<#ty>()) }
    }
}

pub(crate) fn hint_tokens(shape: &TypeShape) -> TokenStream {
    match shape {
        TypeShape::Unit => quote! { ::vela_engine::native::TypeHint::unit() },
        TypeShape::Bool => quote! { ::vela_engine::native::TypeHint::boolean() },
        TypeShape::Char => quote! { ::vela_engine::native::TypeHint::char() },
        TypeShape::I8 => quote! { ::vela_engine::native::TypeHint::i8() },
        TypeShape::I16 => quote! { ::vela_engine::native::TypeHint::i16() },
        TypeShape::I32 => quote! { ::vela_engine::native::TypeHint::i32() },
        TypeShape::I64 => quote! { ::vela_engine::native::TypeHint::i64() },
        TypeShape::U8 => quote! { ::vela_engine::native::TypeHint::u8() },
        TypeShape::U16 => quote! { ::vela_engine::native::TypeHint::u16() },
        TypeShape::U32 => quote! { ::vela_engine::native::TypeHint::u32() },
        TypeShape::U64 => quote! { ::vela_engine::native::TypeHint::u64() },
        TypeShape::F32 => quote! { ::vela_engine::native::TypeHint::f32() },
        TypeShape::F64 => quote! { ::vela_engine::native::TypeHint::f64() },
        TypeShape::String => quote! { ::vela_engine::native::TypeHint::string() },
        TypeShape::Array(element) => {
            let element = hint_tokens(element);
            quote! { ::vela_engine::native::TypeHint::array_of(#element) }
        }
        TypeShape::Map(key, value) => {
            let key = hint_tokens(key);
            let value = hint_tokens(value);
            quote! { ::vela_engine::native::TypeHint::map_of(#key, #value) }
        }
        TypeShape::Set(element) => {
            let element = hint_tokens(element);
            quote! { ::vela_engine::native::TypeHint::set_of(#element) }
        }
        TypeShape::Tuple(elements) => {
            let elements = elements.iter().map(hint_tokens);
            quote! { ::vela_engine::native::TypeHint::tuple_of([#(#elements),*]) }
        }
        TypeShape::Option(payload) => {
            let payload = hint_tokens(payload);
            quote! { ::vela_engine::native::TypeHint::option_of(#payload) }
        }
        TypeShape::Result(ok, err) => {
            let ok = hint_tokens(ok);
            let err = hint_tokens(err);
            quote! { ::vela_engine::native::TypeHint::result_of(#ok, #err) }
        }
        TypeShape::Value(ty) => {
            quote! { <#ty as ::vela_engine::interop::VelaValueBoundary>::vela_type_hint() }
        }
        TypeShape::StorageDirectedShared(ty) => {
            quote! {
                <#ty as ::vela_engine::interop::VelaSharedBoundary>::vela_shared_type_hint()
            }
        }
        TypeShape::Host(ty, _) => {
            quote! { <#ty as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_hint() }
        }
        TypeShape::BorrowedCollection(collection) => {
            let mutable = collection.access == HostAccess::Exclusive;
            let mutation = match collection.mutation {
                vela_common::CollectionViewMutation::Fixed => {
                    quote! { ::vela_common::CollectionViewMutation::Fixed }
                }
                vela_common::CollectionViewMutation::Growable => {
                    quote! { ::vela_common::CollectionViewMutation::Growable }
                }
            };
            match &collection.kind {
                BorrowedCollectionKind::Array(element) => {
                    let element = hint_tokens(element);
                    if mutable {
                        quote! { ::vela_engine::native::TypeHint::array_mut_of(#element, #mutation) }
                    } else {
                        quote! { ::vela_engine::native::TypeHint::array_view_of(#element) }
                    }
                }
                BorrowedCollectionKind::Map(key, value) => {
                    let key = hint_tokens(key);
                    let value = hint_tokens(value);
                    if mutable {
                        quote! { ::vela_engine::native::TypeHint::map_mut_of(#key, #value, #mutation) }
                    } else {
                        quote! { ::vela_engine::native::TypeHint::map_view_of(#key, #value) }
                    }
                }
                BorrowedCollectionKind::Set(element) => {
                    let element = hint_tokens(element);
                    if mutable {
                        quote! { ::vela_engine::native::TypeHint::set_mut_of(#element, #mutation) }
                    } else {
                        quote! { ::vela_engine::native::TypeHint::set_view_of(#element) }
                    }
                }
            }
        }
        TypeShape::ReceiverHost => {
            quote! { <Self as ::vela_engine::interop::VelaHostBoundary>::vela_host_type_hint() }
        }
    }
}
