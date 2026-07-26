use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn base_script_host_object_impl_tokens(self_ty: &syn::Type) -> TokenStream {
    quote! {
        impl ::vela_host::object::ScriptHostObject for #self_ty {
            fn host_type_id(&self) -> ::vela_common::HostTypeId {
                ::vela_host::object::ScriptHostFieldAccess::script_host_type_id(self)
            }

            fn resolve_host_type_target(
                spec: ::vela_host::resolved::HostAccessSpec<'_>,
            ) -> ::vela_host::error::HostResult<::vela_host::resolved::ResolvedHostAccess> {
                if spec.offset < spec.plan.parts.len() {
                    return <Self as ::vela_host::object::ScriptHostFieldAccess>::resolve_host_type_target_from(
                        spec,
                        spec.offset,
                    );
                }
                Ok(::vela_host::resolved::ResolvedHostAccess::generic_target(
                    ::vela_host::resolved::HostSchemaEpoch::new(0),
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
                spec: ::vela_host::resolved::HostAccessSpec<'_>,
            ) -> ::vela_host::error::HostResult<::vela_host::resolved::ResolvedHostAccess> {
                if spec.offset < spec.plan.parts.len() {
                    return ::vela_host::object::ScriptHostFieldAccess::resolve_host_target_from(
                        self,
                        spec,
                        spec.offset,
                    );
                }
                Ok(::vela_host::resolved::ResolvedHostAccess::generic_target(
                    ::vela_host::resolved::HostSchemaEpoch::new(0),
                ))
            }

            fn read_resolved_host(
                &self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                ::vela_host::object::ScriptHostFieldAccess::read_resolved_host_target_from(
                    self,
                    access,
                    target,
                )
            }

            fn borrow_resolved_host_shared(
                &self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<
                Option<::vela_host::lease::ScopedHostDependent<'_>>
            > {
                ::vela_host::object::ScriptHostFieldAccess::borrow_resolved_host_shared(
                    self,
                    access,
                    target,
                )
            }

            fn borrow_resolved_host_exclusive(
                &mut self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<
                Option<::vela_host::lease::ScopedHostDependent<'_>>
            > {
                ::vela_host::object::ScriptHostFieldAccess::borrow_resolved_host_exclusive(
                    self,
                    access,
                    target,
                )
            }

            fn borrow_collection_resolved_host_shared(
                &self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                projection: ::vela_host::protocol::HostCollectionProjection,
            ) -> ::vela_host::error::HostResult<
                Option<::vela_host::object::ScopedHostCollectionDependents<'_>>
            > {
                ::vela_host::object::ScriptHostFieldAccess::
                    borrow_collection_resolved_host_shared(
                        self,
                        access,
                        target,
                        projection,
                    )
            }

            fn borrow_collection_resolved_host_exclusive(
                &mut self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                projection: ::vela_host::protocol::HostCollectionProjection,
            ) -> ::vela_host::error::HostResult<
                Option<::vela_host::object::ScopedHostCollectionDependents<'_>>
            > {
                ::vela_host::object::ScriptHostFieldAccess::
                    borrow_collection_resolved_host_exclusive(
                        self,
                        access,
                        target,
                        projection,
                    )
            }

            fn query_collection_resolved_host(
                &self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                query: ::vela_host::protocol::HostCollectionQuery,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::query_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        query,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let ::vela_host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return ::vela_host::object::ScriptHostFieldAccess::query_prepared_field_target(
                            self,
                            slot,
                            access,
                            target,
                            query,
                        );
                    }
                }
                ::vela_host::object::ScriptHostFieldAccess::query_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    query,
                )
            }

            fn snapshot_collection_resolved_host(
                &self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                projection: ::vela_host::protocol::HostCollectionProjection,
            ) -> ::vela_host::error::HostResult<
                ::vela_host::protocol::HostCollectionSnapshot
            > {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::snapshot_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        projection,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let ::vela_host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return ::vela_host::object::ScriptHostFieldAccess::snapshot_prepared_field_target(
                            self,
                            slot,
                            access,
                            target,
                            projection,
                        );
                    }
                }
                ::vela_host::object::ScriptHostFieldAccess::snapshot_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    projection,
                )
            }

            fn mutate_collection_resolved_host(
                &mut self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                mutation: ::vela_host::protocol::HostCollectionMutation<'_>,
            ) -> ::vela_host::error::HostResult<()> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::mutate_collection_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        mutation,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let ::vela_host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return ::vela_host::object::ScriptHostFieldAccess::mutate_collection_prepared_field_target(
                            self,
                            slot,
                            access,
                            target,
                            mutation,
                        );
                    }
                }
                ::vela_host::object::ScriptHostFieldAccess::mutate_collection_host_target_from(
                    self,
                    target,
                    target.offset,
                    mutation,
                )
            }

            fn remove_resolved_host(
                &mut self,
                _access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<()> {
                ::vela_host::object::ScriptHostFieldAccess::remove_host_target_from(
                    self,
                    target,
                    target.offset,
                )
            }

            fn write_resolved_host(
                &mut self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                value: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                ::vela_host::object::ScriptHostFieldAccess::write_resolved_host_target_from(
                    self,
                    access,
                    target,
                    value,
                )
            }

            fn mutate_resolved_host(
                &mut self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                op: ::vela_host::resolved::HostMutationOp,
                rhs: ::vela_host::value::HostValue,
            ) -> ::vela_host::error::HostResult<()> {
                ::vela_host::object::ScriptHostFieldAccess::mutate_resolved_host_target_from(
                    self,
                    access,
                    target,
                    op,
                    rhs,
                )
            }

            fn call_resolved_host(
                &mut self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
                method: ::vela_common::HostMethodId,
                args: &[::vela_host::value::HostValue],
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::call_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        method,
                        args,
                    );
                }
                if target.offset < target.plan.parts.len() {
                    return ::vela_host::object::ScriptHostFieldAccess::call_host_target_from(
                        self,
                        target,
                        target.offset,
                        method,
                        args,
                    );
                }
                if let ::vela_host::resolved::ResolvedHostAccessKind::DirectMethod(slot) =
                    access.adapter_kind
                {
                    return match slot {

                        _ => Err(::vela_host::error::HostError {
                            kind: ::vela_host::error::HostErrorKind::UnsupportedMethod { method },
                            source_span: None,
                        }),
                    };
                }
                match method {

                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::UnsupportedMethod { method },
                        source_span: None,
                    }),
                }
            }
        }
    }
}
