use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::schema::{FieldMeta, VariantMeta};

pub(super) fn field_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
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
        ::vela_reflect::registry::FieldDesc::new(::vela_def::FieldId::new(#id), #script_name)
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
    let id = u128::from(field.id);
    let field_id_ident = format_ident!("vela_field_id_{}", field.rust_name);
    let field_path_ident = format_ident!("vela_field_path_{}", field.rust_name);
    let field_proxy_ident = format_ident!("vela_field_proxy_{}", field.rust_name);

    quote! {
        #[must_use]
        pub const fn #field_id_ident() -> ::vela_def::FieldId {
            ::vela_def::FieldId::new(#id)
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
    let direct_read_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable)
        .map(direct_field_read_arm_tokens);
    let direct_write_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable || field.writable)
        .map(direct_field_write_arm_tokens);
    let resolve_arms = fields.iter().enumerate().map(field_resolve_arm_tokens);
    let read_arms = fields
        .iter()
        .filter(|field| field.readable)
        .map(field_read_arm_tokens);
    let write_arms = fields
        .iter()
        .filter(|field| field.readable || field.writable)
        .map(field_write_arm_tokens);
    let query_arms = fields
        .iter()
        .filter(|field| field.readable)
        .map(field_query_arm_tokens);
    let snapshot_arms = fields
        .iter()
        .filter(|field| field.readable)
        .map(field_snapshot_arm_tokens);
    let collection_mutation_arms = fields
        .iter()
        .filter(|field| field.readable || field.writable)
        .map(field_collection_mutation_arm_tokens);
    let remove_arms = fields
        .iter()
        .filter(|field| field.readable || field.writable)
        .map(field_remove_arm_tokens);
    let call_arms = fields.iter().map(field_call_arm_tokens);
    let prepared_call_arms = fields
        .iter()
        .enumerate()
        .map(prepared_field_call_arm_tokens);
    let prepared_read_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable)
        .map(prepared_field_read_arm_tokens);
    let prepared_write_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable || field.writable)
        .map(prepared_field_write_arm_tokens);
    let prepared_mutate_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable || field.writable)
        .map(prepared_field_mutate_arm_tokens);
    let prepared_query_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable)
        .map(prepared_field_query_arm_tokens);
    let prepared_snapshot_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable)
        .map(prepared_field_snapshot_arm_tokens);
    let prepared_collection_mutation_arms = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.readable || field.writable)
        .map(prepared_field_collection_mutation_arm_tokens);

    quote! {
        impl ::vela_host::object::ScriptHostFieldAccess for #ident {
            fn script_host_type_id(&self) -> ::vela_common::HostTypeId {
                Self::vela_host_type_id()
            }

            fn read_direct_field(
                &self,
                slot: u32,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                match slot {
                    #(#direct_read_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn write_direct_field(
                &mut self,
                slot: u32,
                target: ::vela_host::target::HostTargetInstance<'_>,
                value: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                match slot {
                    #(#direct_write_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                            path: target.to_diagnostic_path().to_host_path(),
                            action: "write",
                        },
                        source_span: None,
                    }),
                }
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

            fn query_collection_host_target_from(
                &self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
                query: ::vela_host::protocol::HostCollectionQuery,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                match target.plan.parts.as_slice().get(offset) {
                    #(#query_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn snapshot_collection_host_target_from(
                &self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
                projection: ::vela_host::protocol::HostCollectionProjection,
            ) -> ::vela_host::error::HostResult<
                ::vela_host::protocol::HostCollectionSnapshot
            > {
                match target.plan.parts.as_slice().get(offset) {
                    #(#snapshot_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn mutate_collection_host_target_from(
                &mut self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
                mutation: ::vela_host::protocol::HostCollectionMutation<'_>,
            ) -> ::vela_host::error::HostResult<()> {
                match target.plan.parts.as_slice().get(offset) {
                    #(#collection_mutation_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn remove_host_target_from(
                &mut self,
                target: ::vela_host::target::HostTargetInstance<'_>,
                offset: usize,
            ) -> ::vela_host::error::HostResult<()> {
                match target.plan.parts.as_slice().get(offset) {
                    #(#remove_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
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

            fn call_prepared_field_target(
                &mut self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                method: ::vela_common::HostMethodId,
                args: &[::vela_host::value::HostValue],
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                match slot {
                    #(#prepared_call_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn read_prepared_field_target(
                &self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                match slot {
                    #(#prepared_read_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn write_prepared_field_target(
                &mut self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                value: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                match slot {
                    #(#prepared_write_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                            path: target.to_diagnostic_path().to_host_path(),
                            action: "write",
                        },
                        source_span: None,
                    }),
                }
            }

            fn mutate_prepared_field_target(
                &mut self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                op: ::vela_host::resolved::HostMutationOp,
                rhs: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                match slot {
                    #(#prepared_mutate_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                            path: target.to_diagnostic_path().to_host_path(),
                            action: "mutate",
                        },
                        source_span: None,
                    }),
                }
            }

            fn query_prepared_field_target(
                &self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                query: ::vela_host::protocol::HostCollectionQuery,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                match slot {
                    #(#prepared_query_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn snapshot_prepared_field_target(
                &self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                projection: ::vela_host::protocol::HostCollectionProjection,
            ) -> ::vela_host::error::HostResult<
                ::vela_host::protocol::HostCollectionSnapshot
            > {
                match slot {
                    #(#prepared_snapshot_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::MissingPath {
                            path: target.to_diagnostic_path().to_host_path(),
                        },
                        source_span: None,
                    }),
                }
            }

            fn mutate_collection_prepared_field_target(
                &mut self,
                slot: u32,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                mutation: ::vela_host::protocol::HostCollectionMutation<'_>,
            ) -> ::vela_host::error::HostResult<()> {
                match slot {
                    #(#prepared_collection_mutation_arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::PermissionDenied {
                            path: target.to_diagnostic_path().to_host_path(),
                            action: "mutate collection",
                        },
                        source_span: None,
                    }),
                }
            }
        }
    }
}

fn direct_field_read_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostFieldAccess::read_host_target_from(
            &self.#rust_name,
            target,
            target.offset + 1,
        ),
    }
}

fn direct_field_write_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let writable = field.writable;
    let rust_name = format_ident!("{}", field.rust_name);
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
                &mut self.#rust_name,
                target,
                target.offset + 1,
                value,
            )
        }
    }
}

fn field_resolve_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let id = u128::from(field.id);
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
        {
            if offset + 1 == spec.plan.parts.len()
                && !matches!(spec.op, ::vela_host::resolved::HostAccessOp::Call(_))
            {
                Ok(::vela_host::resolved::ResolvedHostAccess::direct_field(
                    #slot,
                    ::vela_host::resolved::HostSchemaEpoch::new(0),
                ))
            } else if matches!(
                spec.op,
                ::vela_host::resolved::HostAccessOp::Call(_)
            ) {
                let __vela_child_access =
                    ::vela_host::object::ScriptHostObject::resolve_host_target(
                    &self.#rust_name,
                    spec.at_offset(offset + 1),
                )?;
                Ok(__vela_child_access.prepend_prepared_field(#slot))
            } else {
                let __vela_child_access =
                    ::vela_host::object::ScriptHostFieldAccess::resolve_host_target_from(
                    &self.#rust_name,
                    spec,
                    offset + 1,
                )?;
                Ok(__vela_child_access.prepend_prepared_field(#slot))
            }
        }
    }
}

fn field_read_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
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
    let id = u128::from(field.id);
    let writable = field.writable;
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
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

fn field_query_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
        {
            ::vela_host::object::ScriptHostFieldAccess::query_collection_host_target_from(
                &self.#rust_name,
                target,
                offset + 1,
                query,
            )
        }
    }
}

fn field_snapshot_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
        {
            ::vela_host::object::ScriptHostFieldAccess::snapshot_collection_host_target_from(
                &self.#rust_name,
                target,
                offset + 1,
                projection,
            )
        }
    }
}

fn field_collection_mutation_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
        {
            ::vela_host::object::ScriptHostFieldAccess::mutate_collection_host_target_from(
                &mut self.#rust_name,
                target,
                offset + 1,
                mutation,
            )
        }
    }
}

fn field_remove_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
        {
            ::vela_host::object::ScriptHostFieldAccess::remove_host_target_from(
                &mut self.#rust_name,
                target,
                offset + 1,
            )
        }
    }
}

fn field_call_arm_tokens(field: &FieldMeta) -> TokenStream {
    let id = u128::from(field.id);
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        Some(::vela_host::target::HostPathPart::Field(field))
            if *field == ::vela_def::FieldId::new(#id) =>
        {
            let __vela_child_spec = ::vela_host::resolved::HostAccessSpec::new(
                ::vela_host::resolved::HostAccessOp::Call(method),
                target.plan,
            )
            .at_offset(offset + 1);
            let __vela_child_access =
                ::vela_host::object::ScriptHostObject::resolve_host_target(
                    &self.#rust_name,
                    __vela_child_spec,
                )?;
            ::vela_host::object::ScriptHostObject::call_resolved_host(
                &mut self.#rust_name,
                __vela_child_access,
                target.at_offset(offset + 1),
                method,
                args,
            )
        }
    }
}

fn prepared_field_call_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::call_resolved_host(
            &mut self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
            method,
            args,
        ),
    }
}

fn prepared_field_read_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::read_resolved_host(
            &self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
        ),
    }
}

fn prepared_field_write_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::write_resolved_host(
            &mut self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
            value,
        ),
    }
}

fn prepared_field_mutate_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::mutate_resolved_host(
            &mut self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
            op,
            rhs,
        ),
    }
}

fn prepared_field_query_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::query_collection_resolved_host(
            &self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
            query,
        ),
    }
}

fn prepared_field_snapshot_arm_tokens((slot, field): (usize, &FieldMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::snapshot_collection_resolved_host(
            &self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
            projection,
        ),
    }
}

fn prepared_field_collection_mutation_arm_tokens(
    (slot, field): (usize, &FieldMeta),
) -> TokenStream {
    let slot = u32::try_from(slot).expect("host field slot index fits u32");
    let rust_name = format_ident!("{}", field.rust_name);
    quote! {
        #slot => ::vela_host::object::ScriptHostObject::mutate_collection_resolved_host(
            &mut self.#rust_name,
            access,
            target.at_offset(target.offset + 1),
            mutation,
        ),
    }
}

pub(super) fn variant_tokens(variant: &VariantMeta) -> TokenStream {
    let id = u128::from(variant.id);
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
            ::vela_def::VariantId::new(#id),
            #script_name,
        )
        #(#attr_tokens)*
        #docs_tokens
        #(
            .field(#field_tokens)
        )*
    }
}
