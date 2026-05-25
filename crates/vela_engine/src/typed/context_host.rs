use vela_vm::{Value, VmResult};

use crate::{FromScriptArg, NativeCallContext};

use super::{IntoNativeReturn, TypedContextHostNativeFunction, expect_arity};

impl<F, R> TypedContextHostNativeFunction<()> for F
where
    F: for<'ctx, 'host> Fn(&mut NativeCallContext<'ctx, 'host>) -> R + Send + Sync + 'static,
    R: IntoNativeReturn,
{
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value> {
        expect_arity(args, 0)?;
        (self)(ctx).into_native_return()
    }
}

impl<F, A, R> TypedContextHostNativeFunction<(A,)> for F
where
    F: for<'ctx, 'host> Fn(&mut NativeCallContext<'ctx, 'host>, A) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value> {
        expect_arity(args, 1)?;
        (self)(ctx, A::from_script_arg(&args[0])?).into_native_return()
    }
}

impl<F, A, B, R> TypedContextHostNativeFunction<(A, B)> for F
where
    F: for<'ctx, 'host> Fn(&mut NativeCallContext<'ctx, 'host>, A, B) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value> {
        expect_arity(args, 2)?;
        (self)(
            ctx,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
        )
        .into_native_return()
    }
}

impl<F, A, B, C, R> TypedContextHostNativeFunction<(A, B, C)> for F
where
    F: for<'ctx, 'host> Fn(&mut NativeCallContext<'ctx, 'host>, A, B, C) -> R
        + Send
        + Sync
        + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value> {
        expect_arity(args, 3)?;
        (self)(
            ctx,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
        )
        .into_native_return()
    }
}

impl<F, A, B, C, D, R> TypedContextHostNativeFunction<(A, B, C, D)> for F
where
    F: for<'ctx, 'host> Fn(&mut NativeCallContext<'ctx, 'host>, A, B, C, D) -> R
        + Send
        + Sync
        + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    D: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value> {
        expect_arity(args, 4)?;
        (self)(
            ctx,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
            D::from_script_arg(&args[3])?,
        )
        .into_native_return()
    }
}
