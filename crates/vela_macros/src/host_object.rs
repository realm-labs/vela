use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn base_script_host_object_impl_tokens(self_ty: &syn::Type) -> TokenStream {
    base_script_host_object_impl_tokens_with_path(self_ty, quote!(::vela_host))
}

pub(crate) fn base_script_host_object_impl_tokens_with_path(
    self_ty: &syn::Type,
    host: TokenStream,
) -> TokenStream {
    quote! {
        impl #host::object::ScriptHostObject for #self_ty {
            fn host_type_id(&self) -> ::vela_common::HostTypeId {
                #host::object::ScriptHostFieldAccess::script_host_type_id(self)
            }

            fn resolve_host_type_target(
                spec: #host::resolved::HostAccessSpec<'_>,
            ) -> #host::error::HostResult<#host::resolved::ResolvedHostAccess> {
                if spec.offset < spec.plan.parts.len() {
                    return <Self as #host::object::ScriptHostFieldAccess>::resolve_host_type_target_from(
                        spec,
                        spec.offset,
                    );
                }
                Ok(#host::resolved::ResolvedHostAccess::generic_target(
                    #host::resolved::HostSchemaEpoch::new(0),
                ))
            }

            fn lease_any(&self) -> Option<&dyn ::core::any::Any> {
                Some(self)
            }

            fn lease_any_mut(&mut self) -> Option<&mut dyn ::core::any::Any> {
                Some(self)
            }

            fn resolve_host_target(
                &self,
                spec: #host::resolved::HostAccessSpec<'_>,
            ) -> #host::error::HostResult<#host::resolved::ResolvedHostAccess> {
                if spec.offset < spec.plan.parts.len() {
                    return #host::object::ScriptHostFieldAccess::resolve_host_target_from(
                        self,
                        spec,
                        spec.offset,
                    );
                }
                Ok(#host::resolved::ResolvedHostAccess::generic_target(
                    #host::resolved::HostSchemaEpoch::new(0),
                ))
            }

            fn read_resolved_host(
                &self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
            ) -> #host::error::HostResult<#host::value::HostValue> {
                #host::object::ScriptHostFieldAccess::read_resolved_host_target_from(
                    self,
                    access,
                    target,
                )
            }

            fn borrow_resolved_host_shared(
                &self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
            ) -> #host::error::HostResult<
                Option<#host::lease::ScopedHostDependent<'_>>
            > {
                #host::object::ScriptHostFieldAccess::borrow_resolved_host_shared(
                    self,
                    access,
                    target,
                )
            }

            fn borrow_resolved_host_exclusive(
                &mut self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
            ) -> #host::error::HostResult<
                Option<#host::lease::ScopedHostDependent<'_>>
            > {
                #host::object::ScriptHostFieldAccess::borrow_resolved_host_exclusive(
                    self,
                    access,
                    target,
                )
            }

            fn borrow_collection_resolved_host_shared(
                &self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                projection: #host::protocol::HostCollectionProjection,
            ) -> #host::error::HostResult<
                Option<#host::object::ScopedHostCollectionDependents<'_>>
            > {
                #host::object::ScriptHostFieldAccess::
                    borrow_collection_resolved_host_shared(
                        self,
                        access,
                        target,
                        projection,
                    )
            }

            fn borrow_collection_resolved_host_exclusive(
                &mut self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                projection: #host::protocol::HostCollectionProjection,
            ) -> #host::error::HostResult<
                Option<#host::object::ScopedHostCollectionDependents<'_>>
            > {
                #host::object::ScriptHostFieldAccess::
                    borrow_collection_resolved_host_exclusive(
                        self,
                        access,
                        target,
                        projection,
                    )
            }

            fn query_collection_resolved_host(
                &self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                query: #host::protocol::HostCollectionQuery,
            ) -> #host::error::HostResult<#host::value::HostValue> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return #host::object::ScriptHostFieldAccess::query_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        query,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let #host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return #host::object::ScriptHostFieldAccess::query_prepared_field_target(
                            self,
                            slot,
                            access,
                            target,
                            query,
                        );
                    }
                }
                #host::object::ScriptHostFieldAccess::query_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    query,
                )
            }

            fn snapshot_collection_resolved_host(
                &self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                projection: #host::protocol::HostCollectionProjection,
            ) -> #host::error::HostResult<
                #host::protocol::HostCollectionSnapshot
            > {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return #host::object::ScriptHostFieldAccess::snapshot_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        projection,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let #host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return #host::object::ScriptHostFieldAccess::snapshot_prepared_field_target(
                            self,
                            slot,
                            access,
                            target,
                            projection,
                        );
                    }
                }
                #host::object::ScriptHostFieldAccess::snapshot_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    projection,
                )
            }

            fn mutate_collection_resolved_host(
                &mut self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                mutation: #host::protocol::HostCollectionMutation<'_>,
            ) -> #host::error::HostResult<()> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return #host::object::ScriptHostFieldAccess::mutate_collection_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        mutation,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let #host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return #host::object::ScriptHostFieldAccess::mutate_collection_prepared_field_target(
                            self,
                            slot,
                            access,
                            target,
                            mutation,
                        );
                    }
                }
                #host::object::ScriptHostFieldAccess::mutate_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    mutation,
                )
            }

            fn remove_resolved_host(
                &mut self,
                _access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
            ) -> #host::error::HostResult<()> {
                #host::object::ScriptHostFieldAccess::remove_host_target_from(
                    self,
                    target,
                    target.offset,
                )
            }

            fn write_resolved_host(
                &mut self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                value: #host::value::HostValue,
            ) -> #host::error::HostResult<()> {
                #host::object::ScriptHostFieldAccess::write_resolved_host_target_from(
                    self,
                    access,
                    target,
                    value,
                )
            }

            fn mutate_resolved_host(
                &mut self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                op: #host::resolved::HostMutationOp,
                rhs: #host::value::HostValue,
            ) -> #host::error::HostResult<()> {
                #host::object::ScriptHostFieldAccess::mutate_resolved_host_target_from(
                    self,
                    access,
                    target,
                    op,
                    rhs,
                )
            }

            fn call_resolved_host(
                &mut self,
                access: #host::resolved::ResolvedHostAccess,
                target: #host::target::HostTargetInstance<'_>,
                method: ::vela_common::HostMethodId,
                args: &[#host::call_value::HostCallValue],
            ) -> #host::error::HostResult<#host::call_value::HostCallValue> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return #host::object::ScriptHostFieldAccess::call_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        method,
                        args,
                    );
                }
                if target.offset < target.plan.parts.len() {
                    return #host::object::ScriptHostFieldAccess::call_host_target_from(
                        self,
                        target,
                        target.offset,
                        method,
                        args,
                    );
                }
                if let #host::resolved::ResolvedHostAccessKind::DirectMethod(slot) =
                    access.adapter_kind
                {
                    return match slot {

                        _ => Err(#host::error::HostError {
                            kind: #host::error::HostErrorKind::UnsupportedMethod { method },
                            source_span: None,
                        }),
                    };
                }
                match method {

                    _ => Err(#host::error::HostError {
                        kind: #host::error::HostErrorKind::UnsupportedMethod { method },
                        source_span: None,
                    }),
                }
            }
        }
    }
}
