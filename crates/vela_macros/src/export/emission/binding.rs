use proc_macro2::TokenStream;
use quote::quote;

use crate::export::signature::TypeShape;

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
    }
}
