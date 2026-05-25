//! Derive macros for Vela host embedding metadata.

mod attrs;
mod hash;
mod script_function;
mod script_host;
mod script_methods;

use proc_macro::TokenStream;

#[proc_macro_derive(ScriptHost, attributes(script))]
pub fn derive_script_host(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Host).into()
}

#[proc_macro_derive(ScriptReflect, attributes(script))]
pub fn derive_script_reflect(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Reflect).into()
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
