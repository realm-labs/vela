use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::FieldMeta;

pub(super) fn prepared_field_borrow_shared_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::borrow_resolved_host_shared(
            &self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
        ),
    }
}

pub(super) fn prepared_field_borrow_exclusive_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::borrow_resolved_host_exclusive(
            &mut self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
        ),
    }
}

pub(super) fn prepared_field_collection_borrow_shared_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::
            borrow_collection_resolved_host_shared(
                &self.#rust_name,
                access,
                target.at_offset(target.offset + 1),
                projection,
            ),
    }
}

pub(super) fn prepared_field_collection_borrow_exclusive_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::
            borrow_collection_resolved_host_exclusive(
                &mut self.#rust_name,
                access,
                target.at_offset(target.offset + 1),
                projection,
            ),
    }
}
