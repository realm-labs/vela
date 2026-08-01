use proc_macro2::TokenStream;
use quote::quote;

use super::{FieldMeta, exclusive_field_access_tokens, shared_field_access_tokens};

pub(super) fn prepared_field_borrow_shared_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = shared_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::borrow_resolved_host_shared(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
        ),
    }
}

pub(super) fn prepared_field_borrow_exclusive_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::borrow_resolved_host_exclusive(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
        ),
    }
}

pub(super) fn prepared_field_collection_borrow_shared_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = shared_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::
            borrow_collection_resolved_host_shared(
                #field_access,
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
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::
            borrow_collection_resolved_host_exclusive(
                #field_access,
                access,
                target.at_offset(target.offset + 1),
                projection,
            ),
    }
}
