mod emission;
mod meta;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, Result, parse2};

use crate::attrs::{error, spanned_error};
use crate::signature::{
    docs_from_attrs, reject_extern_signature, reject_generic_signature, reject_unsafe_signature,
};

use self::meta::{FunctionMode, function_meta, parse_script_function_attrs};

pub(crate) fn expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input, FunctionMode::Pure) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_context(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input, FunctionMode::Context) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_host(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand_result(attr, input, FunctionMode::Host) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn expand_result(attr: TokenStream, input: TokenStream, mode: FunctionMode) -> Result<TokenStream> {
    let item = parse2::<ItemFn>(input)?;
    reject_generic_signature(&item.sig.generics, mode.attr_name())?;
    if item.sig.asyncness.is_some() {
        return Err(spanned_error(
            &item.sig.asyncness,
            &format!("{} does not support async functions", mode.attr_name()),
        ));
    }
    reject_unsafe_signature(&item.sig, mode.attr_name())?;
    reject_extern_signature(&item.sig, mode.attr_name())?;

    let attrs = parse_script_function_attrs(attr)?;
    let id = attrs.id.ok_or_else(|| {
        error(
            item.sig.ident.span(),
            &format!("script functions require {}(id = N)", mode.attr_name()),
        )
    })?;
    let docs = attrs.docs.clone().or_else(|| docs_from_attrs(&item.attrs));
    let meta = function_meta(&item, attrs, id, docs, mode)?;
    let fn_ident = item.sig.ident.clone();
    let desc_name = format_ident!("vela_native_function_desc_{}", fn_ident);
    let register_name = mode.register_helper_ident(&fn_ident);
    let desc_tokens = emission::desc_tokens(&meta);
    let args_tuple = emission::args_tuple_tokens(&meta.params);
    let register_tokens = emission::register_tokens(mode, &args_tuple, &desc_name, &fn_ident);

    Ok(quote! {
        #item

        #[must_use]
        pub fn #desc_name() -> ::vela_engine::native::NativeFunctionDesc {
            #desc_tokens
        }

        #[must_use]
        pub fn #register_name(
            builder: ::vela_engine::builder::EngineBuilder,
        ) -> ::vela_engine::builder::EngineBuilder {
            #register_tokens
        }
    })
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;
    use super::meta::FunctionMode;

    #[test]
    fn rejects_missing_function_id() {
        let error = expand_result(
            quote! {},
            quote! {
                fn grant(amount: i64) -> i64 {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("missing function id should fail macro expansion");

        assert!(error.to_string().contains("script functions require"));
    }

    #[test]
    fn rejects_generic_functions() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn identity<T>(value: T) -> T {
                    value
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("generic function should fail macro expansion");

        assert!(error.to_string().contains("generic parameters"));
    }

    #[test]
    fn rejects_empty_function_names() {
        let error = expand_result(
            quote! { id = 1, name = "" },
            quote! {
                fn grant(amount: i64) -> i64 {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("empty function name should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script_function name must be a non-empty dotted name")
        );
    }

    #[test]
    fn rejects_malformed_function_names() {
        for name in [".grant", "game.", "game..grant"] {
            let error = expand_result(
                quote! { id = 1, name = #name },
                quote! {
                    fn grant(amount: i64) -> i64 {
                        amount
                    }
                },
                FunctionMode::Pure,
            )
            .expect_err("malformed function name should fail macro expansion");

            assert!(
                error
                    .to_string()
                    .contains("script_function name must be a non-empty dotted name")
            );
        }
    }

    #[test]
    fn rejects_empty_function_permissions() {
        let error = expand_result(
            quote! { id = 1, permission = "" },
            quote! {
                fn grant(amount: i64) -> i64 {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("empty function permission should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script_function permission cannot be empty")
        );
    }

    #[test]
    fn rejects_function_where_clauses() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn grant(amount: i64) -> i64
                where
                    i64: Copy,
                {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("function where clause should fail macro expansion");

        assert!(error.to_string().contains("where clauses"));
    }

    #[test]
    fn rejects_unsafe_functions() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                unsafe fn grant(amount: i64) -> i64 {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("unsafe function should fail macro expansion");

        assert!(error.to_string().contains("unsafe functions"));
    }

    #[test]
    fn rejects_extern_functions() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                extern "C" fn grant(amount: i64) -> i64 {
                    amount
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("extern function should fail macro expansion");

        assert!(error.to_string().contains("extern ABI functions"));
    }

    #[test]
    fn rejects_context_functions_without_context_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn emit_event(amount: i64) -> bool {
                    amount > 0
                }
            },
            FunctionMode::Context,
        )
        .expect_err("missing context parameter should fail macro expansion");

        assert!(error.to_string().contains("NativeCallContext"));
    }

    #[test]
    fn rejects_context_functions_with_by_value_context_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn emit_event(ctx: NativeCallContext) -> bool {
                    true
                }
            },
            FunctionMode::Context,
        )
        .expect_err("by-value context parameter should fail macro expansion");

        assert!(error.to_string().contains("&mut NativeCallContext"));
    }

    #[test]
    fn rejects_context_functions_with_shared_context_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn emit_event(ctx: &NativeCallContext) -> bool {
                    true
                }
            },
            FunctionMode::Context,
        )
        .expect_err("shared context parameter should fail macro expansion");

        assert!(error.to_string().contains("&mut NativeCallContext"));
    }

    #[test]
    fn rejects_host_functions_without_host_execution_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn write_host(amount: i64) -> bool {
                    amount > 0
                }
            },
            FunctionMode::Host,
        )
        .expect_err("missing host execution parameter should fail macro expansion");

        assert!(error.to_string().contains("HostExecution"));
    }

    #[test]
    fn rejects_host_functions_with_by_value_host_execution_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn write_host(host: HostExecution) -> bool {
                    true
                }
            },
            FunctionMode::Host,
        )
        .expect_err("by-value host execution parameter should fail macro expansion");

        assert!(error.to_string().contains("&mut HostExecution"));
    }

    #[test]
    fn rejects_host_functions_with_shared_host_execution_param() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn write_host(host: &HostExecution) -> bool {
                    true
                }
            },
            FunctionMode::Host,
        )
        .expect_err("shared host execution parameter should fail macro expansion");

        assert!(error.to_string().contains("&mut HostExecution"));
    }

    #[test]
    fn rejects_script_visible_rust_reference_parameters() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn mutate_player(player: &mut Player) {}
            },
            FunctionMode::Pure,
        )
        .expect_err("script-visible Rust references should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible parameters cannot use Rust references")
        );
    }

    #[test]
    fn rejects_nested_script_visible_rust_reference_parameters() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn grant(names: Option<&str>) {}
            },
            FunctionMode::Pure,
        )
        .expect_err("nested script-visible Rust references should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible parameters cannot use Rust references")
        );
    }

    #[test]
    fn rejects_script_visible_rust_reference_returns() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn label() -> Option<&'static str> {
                    Some("gold")
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("script-visible Rust reference returns should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible returns cannot use Rust references")
        );
    }

    #[test]
    fn rejects_unsupported_integer_parameters() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn grant(amount: u64) -> i64 {
                    i64::try_from(amount).unwrap_or(0)
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("unsupported integer parameter should fail macro expansion");

        assert!(error.to_string().contains("u64"));
        assert!(
            error
                .to_string()
                .contains("script-visible native signatures do not support")
        );
    }

    #[test]
    fn rejects_unsupported_integer_parameters_inside_arrays() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn grant(amounts: [usize; 2]) -> i64 {
                    0
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("unsupported array integer parameter should fail macro expansion");

        assert!(error.to_string().contains("usize"));
    }

    #[test]
    fn rejects_unsupported_integer_returns() {
        let error = expand_result(
            quote! { id = 1 },
            quote! {
                fn grant() -> Option<usize> {
                    Some(1)
                }
            },
            FunctionMode::Pure,
        )
        .expect_err("unsupported integer return should fail macro expansion");

        assert!(error.to_string().contains("usize"));
    }

    #[test]
    fn infers_fixed_array_signature_hints() {
        let tokens = expand_result(
            quote! { id = 1 },
            quote! {
                fn weights(values: [i64; 3]) -> [i64; 3] {
                    values
                }
            },
            FunctionMode::Pure,
        )
        .expect("fixed array function should expand")
        .to_string();

        assert!(tokens.contains("TypeHint :: Array"));
    }
}
