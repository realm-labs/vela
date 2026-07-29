#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Derive macros for Vela host embedding metadata.

mod attrs;
mod export;
mod export_external_trait_impl;
mod export_module;
mod external_host;
mod external_value_enum;
mod hash;
mod host_object;
mod methods;
mod script_host;
mod service;
mod service_domain;
mod service_domain_input;
mod signature;
mod trait_export;
mod value;

use proc_macro::TokenStream;

/// Describes one ordinary Rust function through the unified Rust/Vela callable
/// contract. Adapter registration uses the same classified signature.
#[proc_macro_attribute]
pub fn export(attr: TokenStream, input: TokenStream) -> TokenStream {
    export::expand(attr.into(), input.into()).into()
}

/// Exports the supported public functions in one explicit inline module.
#[proc_macro_attribute]
pub fn export_module(attr: TokenStream, input: TokenStream) -> TokenStream {
    export_module::expand(attr.into(), input.into()).into()
}

/// Exports the supported public methods in one explicit inherent impl group.
#[proc_macro_attribute]
pub fn methods(attr: TokenStream, input: TokenStream) -> TokenStream {
    methods::expand(attr.into(), input.into()).into()
}

/// Generates a centralized Host binding for an existing local Rust type.
///
/// Unlike [`methods`], the supplied impl is registration metadata: its methods
/// are emitted through a private extension trait and do not become inherent
/// methods on the Rust type.
#[proc_macro_attribute]
pub fn external_host(attr: TokenStream, input: TokenStream) -> TokenStream {
    external_host::expand(attr.into(), input.into()).into()
}

/// Generates a centralized structural Value binding for an existing unit enum.
#[proc_macro]
pub fn external_value_enum(input: TokenStream) -> TokenStream {
    external_value_enum::expand(input.into()).into()
}

/// Exports a Rust trait as one explicitly named Vela protocol contract.
#[proc_macro_attribute]
pub fn trait_export(attr: TokenStream, input: TokenStream) -> TokenStream {
    trait_export::expand(attr.into(), input.into()).into()
}

/// Declares one Rust default service trait and its sealed Vela ABI schema.
#[proc_macro_attribute]
pub fn service(attr: TokenStream, input: TokenStream) -> TokenStream {
    service::expand(attr.into(), input.into()).into()
}

/// Generates one atomic service domain configured with Rust default instances.
#[proc_macro_attribute]
pub fn service_domain(attr: TokenStream, input: TokenStream) -> TokenStream {
    service_domain::expand(attr.into(), input.into()).into()
}

/// Generates declaration-only UFCS adapters for an existing external trait
/// implementation without creating a duplicate Rust impl.
#[proc_macro]
pub fn export_external_trait_impl(input: TokenStream) -> TokenStream {
    export_external_trait_impl::expand(input.into()).into()
}

#[proc_macro_derive(ScriptHost, attributes(vela))]
pub fn derive_script_host(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Host).into()
}

#[proc_macro_derive(ScriptReflect, attributes(vela))]
pub fn derive_script_reflect(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Reflect).into()
}

/// Generates a structural Value codec, schema, and unified TypeBinding.
#[proc_macro_derive(Value, attributes(vela))]
pub fn derive_value(input: TokenStream) -> TokenStream {
    value::expand(input.into()).into()
}
