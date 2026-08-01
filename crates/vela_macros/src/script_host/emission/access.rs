use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::FieldMeta;

pub(super) fn shared_field_access_tokens(field: &FieldMeta) -> TokenStream {
    let rust_name = format_ident!("{}", field.rust_name);
    if field.deref {
        quote! { ::std::ops::Deref::deref(&self.#rust_name) }
    } else {
        quote! { &self.#rust_name }
    }
}

pub(super) fn exclusive_field_access_tokens(field: &FieldMeta) -> TokenStream {
    let rust_name = format_ident!("{}", field.rust_name);
    if field.deref {
        quote! { ::std::ops::DerefMut::deref_mut(&mut self.#rust_name) }
    } else {
        quote! { &mut self.#rust_name }
    }
}
