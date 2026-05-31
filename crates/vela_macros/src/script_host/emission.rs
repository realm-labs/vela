use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::schema::FieldMeta;

pub(super) fn field_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let script_name = &field.script_name;
    let rust_name = &field.rust_name;
    let readable = field.readable;
    let writable = field.writable;
    let permission_tokens = field
        .permissions
        .iter()
        .map(|permission| quote! { .require_permission(#permission) });
    let hint_tokens = field
        .type_hint
        .as_ref()
        .map(|hint| quote! { .type_hint(#hint) });
    let docs_tokens = field.docs.as_ref().map(|docs| quote! { .docs(#docs) });
    let attr_tokens = field.attrs.iter().map(|(name, value)| {
        quote! {
            .attr(#name, #value)
        }
    });

    quote! {
        ::vela_reflect::FieldDesc::new(::vela_common::FieldId::new(#id), #script_name)
            .access(
                ::vela_reflect::FieldAccess::new()
                    .readable(#readable)
                    .writable(#writable)
                    .reflect_readable(#readable)
                    .reflect_writable(#writable)
                    #(#permission_tokens)*
            )
            .attr("rust_name", #rust_name)
            #(#attr_tokens)*
            #hint_tokens
            #docs_tokens
    }
}

pub(super) fn field_helper_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let field_id_ident = format_ident!("vela_field_id_{}", field.rust_name);
    let field_path_ident = format_ident!("vela_field_path_{}", field.rust_name);
    let field_proxy_ident = format_ident!("vela_field_proxy_{}", field.rust_name);

    quote! {
        #[must_use]
        pub const fn #field_id_ident() -> ::vela_engine::FieldId {
            ::vela_engine::FieldId::new(#id)
        }

        #[must_use]
        pub fn #field_path_ident(host_ref: ::vela_engine::HostRef) -> ::vela_engine::HostPath {
            ::vela_engine::HostPath::new(host_ref).field(Self::#field_id_ident())
        }

        #[must_use]
        pub fn #field_proxy_ident(host_ref: ::vela_engine::HostRef) -> ::vela_engine::PathProxy {
            ::vela_engine::PathProxy::new(Self::#field_path_ident(host_ref))
        }
    }
}
