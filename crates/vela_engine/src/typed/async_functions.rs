use vela_host::path::HostPath;
use vela_vm::HostExecution;
use vela_vm::error::VmError;
use vela_vm::owned_value::OwnedValue;

use crate::args::FromScriptArg;
use crate::context::NativeCallContext;
use crate::native::NativeCallFuture;

use super::{
    TypedAsyncContextHostNativeFunction, TypedAsyncHostNativeFunction, TypedAsyncNativeFunction,
    TypedAsyncNativeMethodFunction, expect_arity,
};

fn ready_error<'call>(error: VmError) -> NativeCallFuture<'call> {
    Box::pin(async move { Err(error) })
}

macro_rules! convert_args {
    ($args:ident; $(($ty:ident, $value:ident, $index:expr)),* $(,)?) => {
        $(
            let $value = match $ty::from_script_arg(&$args[$index]) {
                Ok(value) => value,
                Err(error) => return ready_error(error),
            };
        )*
    };
}

macro_rules! impl_typed_async_functions {
    ($tuple:ty; $(($ty:ident, $value:ident, $index:expr)),* $(,)?) => {
        impl<F, $($ty),*> TypedAsyncNativeFunction<$tuple> for F
        where
            F: Fn($($ty),*) -> NativeCallFuture<'static> + Send + Sync + 'static,
            $($ty: FromScriptArg,)*
        {
            fn call_async<'call>(
                &self,
                args: &'call [OwnedValue],
            ) -> NativeCallFuture<'call> {
                if let Err(error) = expect_arity(args, impl_typed_async_functions!(@count $($ty),*)) {
                    return ready_error(error);
                }
                convert_args!(args; $(($ty, $value, $index)),*);
                (self)($($value),*)
            }
        }

        impl<F, $($ty),*> TypedAsyncHostNativeFunction<$tuple> for F
        where
            F: for<'call, 'host> Fn(
                    &'call mut HostExecution<'host>,
                    $($ty),*
                ) -> NativeCallFuture<'call>
                + Send
                + Sync
                + 'static,
            $($ty: FromScriptArg,)*
        {
            fn call_async_host<'call, 'host>(
                &self,
                args: &'call [OwnedValue],
                host: &'call mut HostExecution<'host>,
            ) -> NativeCallFuture<'call> {
                if let Err(error) = expect_arity(args, impl_typed_async_functions!(@count $($ty),*)) {
                    return ready_error(error);
                }
                convert_args!(args; $(($ty, $value, $index)),*);
                (self)(host, $($value),*)
            }
        }

        impl<F, $($ty),*> TypedAsyncContextHostNativeFunction<$tuple> for F
        where
            F: for<'call, 'host> Fn(
                    &'call mut NativeCallContext<'call, 'host>,
                    $($ty),*
                ) -> NativeCallFuture<'call>
                + Send
                + Sync
                + 'static,
            $($ty: FromScriptArg,)*
        {
            fn call_async_context<'call, 'host>(
                &self,
                args: &'call [OwnedValue],
                ctx: &'call mut NativeCallContext<'call, 'host>,
            ) -> NativeCallFuture<'call> {
                if let Err(error) = expect_arity(args, impl_typed_async_functions!(@count $($ty),*)) {
                    return ready_error(error);
                }
                convert_args!(args; $(($ty, $value, $index)),*);
                (self)(ctx, $($value),*)
            }
        }

        impl<F, $($ty),*> TypedAsyncNativeMethodFunction<$tuple> for F
        where
            F: for<'call, 'host> Fn(
                    &'call HostPath,
                    &'call mut HostExecution<'host>,
                    $($ty),*
                ) -> NativeCallFuture<'call>
                + Send
                + Sync
                + 'static,
            $($ty: FromScriptArg,)*
        {
            fn call_async_method<'call, 'host>(
                &self,
                receiver: &'call HostPath,
                args: &'call [OwnedValue],
                host: &'call mut HostExecution<'host>,
            ) -> NativeCallFuture<'call> {
                if let Err(error) = expect_arity(args, impl_typed_async_functions!(@count $($ty),*)) {
                    return ready_error(error);
                }
                convert_args!(args; $(($ty, $value, $index)),*);
                (self)(receiver, host, $($value),*)
            }
        }
    };
    (@count $($ty:ident),*) => {
        <[()]>::len(&[$(impl_typed_async_functions!(@unit $ty)),*])
    };
    (@unit $ty:ident) => { () };
}

impl_typed_async_functions!((););
impl_typed_async_functions!((A,); (A, a, 0));
impl_typed_async_functions!((A, B); (A, a, 0), (B, b, 1));
impl_typed_async_functions!((A, B, C); (A, a, 0), (B, b, 1), (C, c, 2));
impl_typed_async_functions!((A, B, C, D); (A, a, 0), (B, b, 1), (C, c, 2), (D, d, 3));
impl_typed_async_functions!(
    (A, B, C, D, E);
    (A, a, 0), (B, b, 1), (C, c, 2), (D, d, 3), (E, e, 4)
);
impl_typed_async_functions!(
    (A, B, C, D, E, G);
    (A, a, 0), (B, b, 1), (C, c, 2), (D, d, 3), (E, e, 4), (G, g, 5)
);
