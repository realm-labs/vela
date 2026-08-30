use proc_macro2::TokenStream;
use quote::quote;

use super::FieldMeta;
use super::access::{exclusive_field_access_tokens, shared_field_access_tokens};

pub(super) fn direct_field_read_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = shared_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostFieldAccess::read_host_target_from(
            #field_access,
            target,
            target.offset + 1,
        ),
    }
}

pub(super) fn direct_field_write_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let writable = field.writable;
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => {
            if !#writable {
                return Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                        path: target.to_diagnostic_path().to_host_path(),
                        action: "write",
                    },
                    source_span: None,
                });
            }
            ::vela_host::object::ScriptHostFieldAccess::write_host_target_from(
                #field_access,
                target,
                target.offset + 1,
                value,
            )
        }
    }
}

pub(super) fn prepared_field_call_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::call_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
            method,
            args,
        ),
    }
}

pub(super) fn prepared_field_read_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = shared_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::read_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
        ),
    }
}

pub(super) fn prepared_field_write_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::write_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
            value,
        ),
    }
}

pub(super) fn prepared_field_mutate_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::mutate_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
            op,
            rhs,
        ),
    }
}

pub(super) fn prepared_field_query_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = shared_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::query_collection_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
            query,
        ),
    }
}

pub(super) fn prepared_field_snapshot_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = shared_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::snapshot_collection_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
            projection,
        ),
    }
}

pub(super) fn prepared_field_collection_mutation_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let field_access = exclusive_field_access_tokens(field);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::mutate_collection_resolved_host(
            #field_access,
            access,
            target.at_offset(target.offset + 1),
            mutation,
        ),
    }
}
