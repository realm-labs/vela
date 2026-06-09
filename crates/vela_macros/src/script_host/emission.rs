use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::schema::{FieldMeta, VariantMeta};

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
        ::vela_reflect::registry::FieldDesc::new(::vela_common::FieldId::new(#id), #script_name)
            .access(
                ::vela_reflect::access::FieldAccess::new()
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
        pub const fn #field_id_ident() -> ::vela_common::FieldId {
            ::vela_common::FieldId::new(#id)
        }

        #[must_use]
        pub fn #field_path_ident(host_ref: ::vela_host::path::HostRef) -> ::vela_host::path::HostPath {
            ::vela_host::path::HostPath::new(host_ref).field(Self::#field_id_ident())
        }

        #[must_use]
        pub fn #field_proxy_ident(host_ref: ::vela_host::path::HostRef) -> ::vela_host::proxy::PathProxy {
            ::vela_host::proxy::PathProxy::new(
                host_ref,
                ::vela_host::target::HostTargetPlan::new(Self::vela_host_type_id())
                    .field(Self::#field_id_ident()),
            )
        }
    }
}

pub(super) fn field_access_impl_tokens(ident: &Ident, fields: &[FieldMeta]) -> TokenStream {
    let resolve_arms = fields.iter().enumerate().map(field_resolve_arm_tokens);
    let read_arms = fields
        .iter()
        .filter(|field| field.readable)
        .map(field_read_arm_tokens);
    let write_arms = fields
        .iter()
        .filter(|field| field.readable || field.writable)
        .map(field_write_arm_tokens);
    let call_arms = fields.iter().map(field_call_arm_tokens);

    quote! {
        impl ::vela_host::object::ScriptHostFieldAccess for #ident {
            fn script_host_type_id(&self) -> ::vela_common::HostTypeId {
                Self::vela_host_type_id()
            }

            fn resolve_host_target_from(
                &self,
                spec: ::vela_host::resolved::HostAccessSpec<'_>,
                offset: usize,
            ) -> ::vela_host::error::HostResult<::vela_host::resolved::ResolvedHostAccess> {
                match spec.plan.parts.as_slice().get(offset) {
                    #(#resolve_arms)*
                    _ => Ok(::vela_host::resolved::ResolvedHostAccess::generic_target(
                        ::vela_host::resolved::HostSchemaEpoch::new(0),
                    )),
                }
            }

            fn read_host_target_from(
                &self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                match target.plan.parts.as_slice().get(offset) {
                    #(#read_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn write_host_target_from(
                &mut self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
                value: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                match target.plan.parts.as_slice().get(offset) {
                    #(#write_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                            path: target.to_diagnostic_path().to_host_path(),
                            action: "write",
                        },
                        source_span: None,
                    }),
                }
            }

            fn mutate_host_target_from(
                &mut self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
                op: ::vela_host::resolved::HostMutationOp,
                rhs: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                let current =
                    ::vela_host::object::ScriptHostFieldAccess::read_host_target_from(
                        self,
                        target,
                        offset,
                    )?;
                let next = ::vela_host::object::mutate_host_value(op, &current, &rhs, target)?;
                ::vela_host::object::ScriptHostFieldAccess::write_host_target_from(
                    self,
                    target,
                    offset,
                    next,
                )
            }

            fn call_host_target_from(
                &mut self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
                method: ::vela_common::HostMethodId,
                args: &[::vela_host::value::HostValue],
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                if offset >= target.plan.parts.len() {
                    return Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::UnsupportedMethod { method },
                        source_span: None,
                    });
                }
                match target.plan.parts.as_slice().get(offset) {
                    #(#call_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }
        }
    }
}

fn field_resolve_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let id = field.id;
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_common::FieldId::new(#id) =>
        {
            if offset + 1 == spec.plan.parts.len()
                && !matches!(spec.op, ::vela_host::resolved::HostAccessOp::Call(_))
            {
                Ok(::vela_host::resolved::ResolvedHostAccess::direct_field(
                    #slot,
                    ::vela_host::resolved::HostSchemaEpoch::new(0),
                ))
            } else if let ::vela_host::resolved::HostAccessOp::Call(method) = spec.op {
                let __vela_child_plan = ::vela_host::target::HostTargetPlan::from_parts(
                    spec.plan.root_type,
                    spec.plan.parts.as_slice()[(offset + 1)..].iter().cloned(),
                );
                let __vela_child_spec = ::vela_host::resolved::HostAccessSpec::new(
                    ::vela_host::resolved::HostAccessOp::Call(method),
                    &__vela_child_plan,
                );
                ::vela_host::object::ScriptHostObject::resolve_host_target(
                    &self.#rust_name,
                    __vela_child_spec,
                )
            } else {
                ::vela_host::object::ScriptHostFieldAccess::resolve_host_target_from(
                    &self.#rust_name,
                    spec,
                    offset + 1,
                )
            }
        }
    }
}

fn field_read_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_common::FieldId::new(#id) =>
        {
            ::vela_host::object::ScriptHostFieldAccess::read_host_target_from(
                &self.#rust_name,
                target,
                offset + 1,
            )
        }
    }
}

fn field_write_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let writable = field.writable;
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_common::FieldId::new(#id) =>
        {
            if offset + 1 == target.plan.parts.len() && !#writable {
                return Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                        path: target.to_diagnostic_path().to_host_path(),
                        action: "write",
                    },
                    source_span: None,
                });
            }
            ::vela_host::object::ScriptHostFieldAccess::write_host_target_from(
                &mut self.#rust_name,
                target,
                offset + 1,
                value,
            )
        }
    }
}

fn field_call_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = field.id;
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_common::FieldId::new(#id) =>
        {
            let __vela_child_plan = ::vela_host::target::HostTargetPlan::from_parts(
                target.plan.root_type,
                target.plan.parts.as_slice()[(offset + 1)..].iter().cloned(),
            );
            let __vela_child_target = ::vela_host::target::HostTargetInstance::new(
                target.root,
                &__vela_child_plan,
                target.args,
            );
            let __vela_child_spec = ::vela_host::resolved::HostAccessSpec::new(
                ::vela_host::resolved::HostAccessOp::Call(method),
                &__vela_child_plan,
            );
            let __vela_child_access =
                ::vela_host::object::ScriptHostObject::resolve_host_target(
                    &self.#rust_name,
                    __vela_child_spec,
                )?;
            ::vela_host::object::ScriptHostObject::call_resolved_host(
                &mut self.#rust_name,
                __vela_child_access,
                __vela_child_target,
                method,
                args,
            )
        }
    }
}

pub(super) fn variant_tokens(variant: &VariantMeta) -> TokenStream {
    let id = variant.id;
    let script_name = &variant.script_name;
    let docs_tokens = variant.docs.as_ref().map(|docs| quote! { .docs(#docs) });
    let attr_tokens = variant.attrs.iter().map(|(name, value)| {
        quote! {
            .attr(#name, #value)
        }
    });
    let field_tokens = variant.fields.iter().map(field_tokens);

    quote! {
        ::vela_reflect::registry::VariantDesc::new(
            ::vela_common::VariantId::new(#id),
            #script_name,
        )
        #(#attr_tokens)*
        #docs_tokens
        #(
            .field(#field_tokens)
        )*
    }
}
