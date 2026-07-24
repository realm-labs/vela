#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Derive macros for Vela host embedding metadata.

mod attrs;
mod export;
mod export_external_trait_impl;
mod export_module;
mod hash;
mod methods;
mod script_function;
mod script_host;
mod script_methods;
mod service;
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

/// Generates declaration-only UFCS adapters for an existing external trait
/// implementation without creating a duplicate Rust impl.
#[proc_macro]
pub fn export_external_trait_impl(input: TokenStream) -> TokenStream {
    export_external_trait_impl::expand(input.into()).into()
}

#[proc_macro_derive(ScriptHost, attributes(script))]
pub fn derive_script_host(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Host).into()
}

#[proc_macro_derive(ScriptReflect, attributes(script))]
pub fn derive_script_reflect(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Reflect).into()
}

/// Generates a structural Value codec, schema, and unified TypeBinding.
#[proc_macro_derive(Value, attributes(script))]
pub fn derive_value(input: TokenStream) -> TokenStream {
    value::expand(input.into()).into()
}

#[proc_macro_attribute]
pub fn script_methods(_attr: TokenStream, input: TokenStream) -> TokenStream {
    script_methods::expand(input.into()).into()
}

#[proc_macro_attribute]
pub fn script_method(_attr: TokenStream, input: TokenStream) -> TokenStream {
    script_methods::expand_standalone_method(input.into()).into()
}

#[proc_macro_attribute]
pub fn script_function(attr: TokenStream, input: TokenStream) -> TokenStream {
    script_function::expand(attr.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn script_context_function(attr: TokenStream, input: TokenStream) -> TokenStream {
    script_function::expand_context(attr.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn script_host_function(attr: TokenStream, input: TokenStream) -> TokenStream {
    script_function::expand_host(attr.into(), input.into()).into()
}
