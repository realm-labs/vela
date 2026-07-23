use proc_macro2::TokenStream;
use quote::quote;
use vela_common::PrimitiveTag;

use super::meta::{HintKind, MethodEffect, MethodMeta, MethodReceiver, ParamMeta};

pub(super) fn method_tokens(method: &MethodMeta) -> TokenStream {
    let desc = method_desc_expr(method);

    quote! {
        methods.push(#desc);
    }
}

fn method_desc_expr(method: &MethodMeta) -> TokenStream {
    let name = &method.name;
    let stable_name = &method.stable_name;
    let effect = effect_tokens(method.effect);
    let returns = hint_tokens(method.returns.clone());
    let params = method.params.iter().map(param_tokens);
    let access = access_tokens(method);
    let receiver = match (method.receiver, method.effect) {
        (MethodReceiver::MutSelf, _) | (MethodReceiver::HostBoundary, MethodEffect::HostWrite) => {
            quote! { ::vela_common::ReceiverCapability::Exclusive }
        }
        (MethodReceiver::SharedSelf | MethodReceiver::HostBoundary, _) => {
            quote! { ::vela_common::ReceiverCapability::Shared }
        }
    };
    let asyncness = if method.is_async {
        quote! { ::vela_common::CallableAsyncness::Async }
    } else {
        quote! { ::vela_common::CallableAsyncness::Sync }
    };
    let docs = method
        .docs
        .as_ref()
        .map(|docs| quote! { desc = desc.docs(#docs); });
    let attrs = method.attrs.iter().map(|(name, value)| {
        quote! {
            desc = desc.attr(#name, #value);
        }
    });

    quote! {{
        let method_id = ::vela_common::HostMethodId::new(::core::primitive::u128::from(::vela_common::stable_id(
            "host_method",
            &owner_stable_path,
            #stable_name,
        )));
        let mut desc = ::vela_engine::method::NativeMethodDesc::new(
            owner_key.clone(),
            method_id,
            #name,
        )
        .effects(#effect)
        .returns(#returns)
        .access(#access)
        .receiver(#receiver)
        .asyncness(#asyncness);
        #(
            desc = desc.param(#params);
        )*
        #(#attrs)*
        #docs
        desc
    }}
}

pub(super) fn native_method_registration_tokens(methods: &[MethodMeta]) -> TokenStream {
    let mut builder = quote! { builder };
    for method in methods.iter().filter(|method| method.callable_native) {
        let desc = method_desc_expr(method);
        let args_tuple = args_tuple_tokens(&method.params);
        let ident = &method.ident;
        builder = quote! {
            #builder.register_typed_native_method_fn::<#args_tuple, _>(
                #desc,
                Self::#ident,
            )
        };
    }

    quote! {
        #builder
    }
}

pub(super) fn script_host_method_registration_tokens(methods: &[MethodMeta]) -> TokenStream {
    let mut builder = quote! { builder };
    for method in methods {
        let desc = method_desc_expr(method);
        if method.is_async {
            let registration = async_direct_method_registration_tokens(method, desc);
            builder = quote! { #builder #registration };
        } else if method.callable_native {
            let args_tuple = args_tuple_tokens(&method.params);
            let ident = &method.ident;
            builder = quote! {
                #builder.register_typed_native_method_fn::<#args_tuple, _>(
                    #desc,
                    Self::#ident,
                )
            };
        } else {
            builder = quote! {
                #builder.register_host_method_desc(#desc)
            };
        }
    }

    quote! {
        #builder
    }
}

pub(super) fn script_host_object_impl_tokens(
    self_ty: &syn::Type,
    methods: &[MethodMeta],
) -> TokenStream {
    let arms = methods
        .iter()
        .filter(|method| method.receiver != MethodReceiver::HostBoundary && !method.is_async)
        .map(host_method_arm_tokens);
    let direct_arms = methods
        .iter()
        .filter(|method| method.receiver != MethodReceiver::HostBoundary)
        .enumerate()
        .filter(|(_, method)| !method.is_async)
        .map(host_method_direct_arm_tokens);
    let resolve_arms = methods
        .iter()
        .filter(|method| method.receiver != MethodReceiver::HostBoundary)
        .enumerate()
        .map(host_method_resolve_arm_tokens);

    quote! {
        impl ::vela_host::object::ScriptHostObject for #self_ty {
            fn host_type_id(&self) -> ::vela_common::HostTypeId {
                ::vela_host::object::ScriptHostFieldAccess::script_host_type_id(self)
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
                let owner_stable_path = Self::vela_stable_type_path();
                match spec.op {
                    #(#resolve_arms)*
                    _ => Ok(::vela_host::resolved::ResolvedHostAccess::generic_target(
                        ::vela_host::resolved::HostSchemaEpoch::new(0),
                    )),
                }
            }

            fn read_resolved_host(
                &self,
                access: ::vela_host::resolved::ResolvedHostAccess,
                target: ::vela_host::target::HostTargetInstance<'_>,
            ) -> ::vela_host::error::HostResult<::vela_host::value::HostValue> {
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::read_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let ::vela_host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return ::vela_host::object::ScriptHostFieldAccess::read_direct_field(
                            self,
                            slot,
                            target,
                        );
                    }
                }
                ::vela_host::object::ScriptHostFieldAccess::read_host_target_from(
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
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::write_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        value,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let ::vela_host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return ::vela_host::object::ScriptHostFieldAccess::write_direct_field(
                            self,
                            slot,
                            target,
                            value,
                        );
                    }
                }
                ::vela_host::object::ScriptHostFieldAccess::write_host_target_from(
                    self,
                    target,
                    target.offset,
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
                if let Some((slot, child_access)) = access.next_prepared_field() {
                    return ::vela_host::object::ScriptHostFieldAccess::mutate_prepared_field_target(
                        self,
                        slot,
                        child_access,
                        target,
                        op,
                        rhs,
                    );
                }
                if target.offset + 1 == target.plan.parts.len() {
                    if let ::vela_host::resolved::ResolvedHostAccessKind::DirectField(slot) =
                        access.adapter_kind
                    {
                        return ::vela_host::object::ScriptHostFieldAccess::mutate_direct_field(
                            self,
                            slot,
                            target,
                            op,
                            rhs,
                        );
                    }
                }
                ::vela_host::object::ScriptHostFieldAccess::mutate_host_target_from(
                    self,
                    target,
                    target.offset,
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
                        #(#direct_arms)*
                        _ => Err(::vela_host::error::HostError {
                            kind: ::vela_host::error::HostErrorKind::UnsupportedMethod { method },
                            source_span: None,
                        }),
                    };
                }
                let owner_stable_path = Self::vela_stable_type_path();
                match method {
                    #(#arms)*
                    _ => Err(::vela_host::error::HostError {
                        kind: ::vela_host::error::HostErrorKind::UnsupportedMethod { method },
                        source_span: None,
                    }),
                }
            }
        }
    }
}

fn async_direct_method_registration_tokens(method: &MethodMeta, desc: TokenStream) -> TokenStream {
    let ident = &method.ident;
    let expected = method.params.len();
    let arg_bindings = method
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| param.host_lease.is_none())
        .map(|(index, param)| {
            let name = quote::format_ident!("__vela_arg_{}", param.name);
            let ty = &param.ty;
            quote! {
                let #name = <#ty as ::vela_engine::args::FromScriptArg>::from_script_arg(
                    &args[#index],
                )?;
            }
        })
        .collect::<Vec<_>>();
    let lease_arg_bindings = method
        .params
        .iter()
        .filter_map(|param| {
            let lease = param.host_lease.as_ref()?;
            let name = quote::format_ident!("__vela_arg_{}", param.name);
            let ty = &lease.ty;
            Some(if lease.mutable {
                quote! {
                    let #name = __vela_leases
                        .next()
                        .and_then(|lease| lease.object_mut())
                        .and_then(|object| object.lease_any_mut())
                        .and_then(|object| object.downcast_mut::<#ty>())
                        .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
                }
            } else {
                quote! {
                    let #name = __vela_leases
                        .next()
                        .and_then(|lease| lease.object().lease_any())
                        .and_then(|object| object.downcast_ref::<#ty>())
                        .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
                }
            })
        })
        .collect::<Vec<_>>();
    let param_leases = method
        .params
        .iter()
        .enumerate()
        .filter_map(|(index, param)| {
            let lease = param.host_lease.as_ref()?;
            let kind = if lease.mutable {
                quote! { ::vela_host::lease::HostLeaseKind::Exclusive }
            } else {
                quote! { ::vela_host::lease::HostLeaseKind::Shared }
            };
            Some(quote! { (#index, #kind) })
        })
        .collect::<Vec<_>>();
    let arg_names = method
        .params
        .iter()
        .map(|param| quote::format_ident!("__vela_arg_{}", param.name))
        .collect::<Vec<_>>();
    let mut call_args = arg_names
        .iter()
        .map(|name| quote! { #name })
        .collect::<Vec<_>>();
    if let Some(index) = method.context_index {
        call_args.insert(index, quote! { __vela_context });
    }
    let (lease_kind, owned_lease, borrowed_lease, owned_receiver, borrowed_receiver) = match method
        .receiver
    {
        MethodReceiver::SharedSelf => (
            quote! { ::vela_host::lease::HostLeaseKind::Shared },
            quote! {
                let __vela_lease = ::vela_engine::host_lease::HostLeaseRef::<Self>::from_erased(
                    lease,
                    root,
                )?;
            },
            quote! {
                let mut __vela_leases = leases.iter_mut();
                let __vela_receiver = __vela_leases
                    .next()
                    .and_then(|lease| lease.object().lease_any())
                    .and_then(|object| object.downcast_ref::<Self>())
                    .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
            },
            quote! { &*__vela_lease },
            quote! { __vela_receiver },
        ),
        MethodReceiver::MutSelf => (
            quote! { ::vela_host::lease::HostLeaseKind::Exclusive },
            quote! {
                let mut __vela_lease = ::vela_engine::host_lease::HostLeaseMut::<Self>::from_erased(
                    lease,
                    root,
                )?;
            },
            quote! {
                let mut __vela_leases = leases.iter_mut();
                let __vela_receiver = __vela_leases
                    .next()
                    .and_then(|lease| lease.object_mut())
                    .and_then(|object| object.lease_any_mut())
                    .and_then(|object| object.downcast_mut::<Self>())
                    .ok_or_else(|| ::vela_host::lease::host_lease_unsupported(root))?;
            },
            quote! { &mut *__vela_lease },
            quote! { __vela_receiver },
        ),
        MethodReceiver::HostBoundary => {
            unreachable!("async host-boundary methods are rejected during metadata collection")
        }
    };

    if method.context_index.is_some() || !param_leases.is_empty() {
        return quote! {
        .register_async_context_direct_method_fn(
            #desc,
            #lease_kind,
            ::std::vec![#(#param_leases),*],
            move |root, leases, args, __vela_context| {
                ::std::boxed::Box::pin(async move {
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: "typed async direct method".to_owned(),
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    #borrowed_lease
                    #(#arg_bindings)*
                    #(#lease_arg_bindings)*
                    let __vela_result = Self::#ident(
                        #borrowed_receiver,
                        #(#call_args),*
                    ).await;
                    ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                })
            },
        )
        };
    }

    quote! {
        .register_async_direct_method_fn(
            #desc,
            #lease_kind,
            move |root, lease, args| {
                ::std::boxed::Box::pin(async move {
                    if args.len() != #expected {
                        return Err(::vela_vm::error::VmError::new(
                            ::vela_vm::error::VmErrorKind::ArityMismatch {
                                name: "typed async direct method".to_owned(),
                                expected: #expected,
                                actual: args.len(),
                            },
                        ));
                    }
                    #owned_lease
                    #(#arg_bindings)*
                    let __vela_result = Self::#ident(
                        #owned_receiver,
                        #(#call_args),*
                    ).await;
                    ::vela_engine::typed::IntoNativeReturn::into_native_return(__vela_result)
                })
            },
        )
    }
}

fn host_method_resolve_arm_tokens((slot, method): (usize, &MethodMeta)) -> TokenStream {
    let stable_name = &method.stable_name;
    let slot = u32::try_from(slot).expect("host method slot index fits u32");
    quote! {
        ::vela_host::resolved::HostAccessOp::Call(method)
            if method == ::vela_common::HostMethodId::new(::core::primitive::u128::from(::vela_common::stable_id(
                "host_method",
                owner_stable_path,
                #stable_name,
            ))) =>
        {
            Ok(::vela_host::resolved::ResolvedHostAccess::direct_method(
                #slot,
                ::vela_host::resolved::HostSchemaEpoch::new(0),
            ))
        }
    }
}

fn host_method_arm_tokens(method: &MethodMeta) -> TokenStream {
    let stable_name = &method.stable_name;
    let call = host_method_call_tokens(method);

    quote! {
        method if method == ::vela_common::HostMethodId::new(::core::primitive::u128::from(::vela_common::stable_id(
            "host_method",
            owner_stable_path,
            #stable_name,
        ))) => {
            #call
        }
    }
}

fn host_method_direct_arm_tokens((slot, method): (usize, &MethodMeta)) -> TokenStream {
    let slot = u32::try_from(slot).expect("host method slot index fits u32");
    let call = host_method_call_tokens(method);
    quote! {
        #slot => {
            #call
        }
    }
}

fn host_method_call_tokens(method: &MethodMeta) -> TokenStream {
    let ident = &method.ident;
    let arg_bindings = method
        .params
        .iter()
        .enumerate()
        .map(host_method_arg_binding_tokens);
    let arg_names = method
        .params
        .iter()
        .map(|param| quote::format_ident!("__vela_arg_{}", param.name));
    let receiver = match method.receiver {
        MethodReceiver::SharedSelf | MethodReceiver::MutSelf => quote! { self },
        MethodReceiver::HostBoundary => {
            unreachable!("host-boundary methods are not direct object methods")
        }
    };

    quote! {
        #(#arg_bindings)*
        let __vela_result = #receiver.#ident(#(#arg_names),*);
        ::vela_host::object::HostValueInto::into_host_value(__vela_result)
    }
}

fn host_method_arg_binding_tokens((index, param): (usize, &ParamMeta)) -> TokenStream {
    let name = quote::format_ident!("__vela_arg_{}", param.name);
    let ty = &param.ty;
    let expected = format!("argument `{}`", param.name);
    quote! {
        let #name = {
            let Some(__vela_value) = args.get(#index) else {
                return Err(::vela_host::error::HostError {
                    kind: ::vela_host::error::HostErrorKind::InvalidArgument {
                        expected: #expected,
                    },
                    source_span: None,
                });
            };
            <#ty as ::vela_host::object::HostValueFrom>::from_host_value(__vela_value)?
        };
    }
}

fn args_tuple_tokens(params: &[ParamMeta]) -> TokenStream {
    match params {
        [] => quote! { () },
        [param] => {
            let ty = &param.ty;
            quote! { (#ty,) }
        }
        params => {
            let types = params.iter().map(|param| &param.ty);
            quote! { (#(#types),*) }
        }
    }
}

fn param_tokens(param: &ParamMeta) -> TokenStream {
    let name = &param.name;
    let hint = if let Some(lease) = &param.host_lease {
        let ty = &lease.ty;
        quote! {
            ::vela_engine::native::TypeHint::Host(
                ::vela_reflect::registry::TypeKey::new(
                    <#ty>::vela_type_id(),
                    ::core::stringify!(#ty),
                )
            )
        }
    } else {
        hint_tokens(param.hint.clone())
    };
    quote! { #name, #hint }
}

fn effect_tokens(effect: MethodEffect) -> TokenStream {
    match effect {
        MethodEffect::Pure => quote! { ::vela_engine::native::EffectSet::pure() },
        MethodEffect::HostRead => quote! { ::vela_engine::native::EffectSet::host_read() },
        MethodEffect::HostWrite => quote! { ::vela_engine::native::EffectSet::host_write() },
        MethodEffect::EventEmit => quote! { ::vela_engine::native::EffectSet::event_emit() },
    }
}

fn hint_tokens(hint: HintKind) -> TokenStream {
    match hint {
        HintKind::Any => quote! { ::vela_engine::native::TypeHint::Any },
        HintKind::Primitive(tag) => primitive_hint_tokens(tag),
        HintKind::Array => quote! { ::vela_engine::native::TypeHint::Array },
        HintKind::ArrayOf(element) => {
            let element = hint_tokens(*element);
            quote! { ::vela_engine::native::TypeHint::array_of(#element) }
        }
        HintKind::Map => quote! { ::vela_engine::native::TypeHint::Map },
        HintKind::MapOf { key, value } => {
            let key = hint_tokens(*key);
            let value = hint_tokens(*value);
            quote! { ::vela_engine::native::TypeHint::map_of(#key, #value) }
        }
        HintKind::Set => quote! { ::vela_engine::native::TypeHint::Set },
        HintKind::SetOf(element) => {
            let element = hint_tokens(*element);
            quote! { ::vela_engine::native::TypeHint::set_of(#element) }
        }
        HintKind::PathProxy => quote! { ::vela_engine::native::TypeHint::PathProxy },
        HintKind::HostOwner => quote! { ::vela_engine::native::TypeHint::Host(owner_key.clone()) },
        HintKind::Function => quote! { ::vela_engine::native::TypeHint::Function },
    }
}

fn primitive_hint_tokens(tag: PrimitiveTag) -> TokenStream {
    match tag {
        PrimitiveTag::Unit => quote! { ::vela_engine::native::TypeHint::unit() },
        PrimitiveTag::Bool => quote! { ::vela_engine::native::TypeHint::boolean() },
        PrimitiveTag::Char => quote! { ::vela_engine::native::TypeHint::char() },
        PrimitiveTag::I8 => quote! { ::vela_engine::native::TypeHint::i8() },
        PrimitiveTag::I16 => quote! { ::vela_engine::native::TypeHint::i16() },
        PrimitiveTag::I32 => quote! { ::vela_engine::native::TypeHint::i32() },
        PrimitiveTag::I64 => quote! { ::vela_engine::native::TypeHint::i64() },
        PrimitiveTag::U8 => quote! { ::vela_engine::native::TypeHint::u8() },
        PrimitiveTag::U16 => quote! { ::vela_engine::native::TypeHint::u16() },
        PrimitiveTag::U32 => quote! { ::vela_engine::native::TypeHint::u32() },
        PrimitiveTag::U64 => quote! { ::vela_engine::native::TypeHint::u64() },
        PrimitiveTag::F32 => quote! { ::vela_engine::native::TypeHint::f32() },
        PrimitiveTag::F64 => quote! { ::vela_engine::native::TypeHint::f64() },
        PrimitiveTag::String => quote! { ::vela_engine::native::TypeHint::string() },
        PrimitiveTag::Bytes => quote! { ::vela_engine::native::TypeHint::bytes() },
    }
}

fn access_tokens(method: &MethodMeta) -> TokenStream {
    let reflect_callable = method.reflect_callable;

    quote! {
        {
            ::vela_engine::native::FunctionAccess::public().reflect_callable(#reflect_callable)
        }
    }
}
