use vela_vm::{Value, VmResult};

use crate::FromScriptArg;

use super::{IntoNativeReturn, TypedNativeFunction, expect_arity};

impl<F, R> TypedNativeFunction<()> for F
where
    F: Fn() -> R + Send + Sync + 'static,
    R: IntoNativeReturn,
{
    fn call(&self, args: &[Value]) -> VmResult<Value> {
        expect_arity(args, 0)?;
        (self)().into_native_return()
    }
}

impl<F, A, R> TypedNativeFunction<(A,)> for F
where
    F: Fn(A) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call(&self, args: &[Value]) -> VmResult<Value> {
        expect_arity(args, 1)?;
        (self)(A::from_script_arg(&args[0])?).into_native_return()
    }
}

impl<F, A, B, R> TypedNativeFunction<(A, B)> for F
where
    F: Fn(A, B) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call(&self, args: &[Value]) -> VmResult<Value> {
        expect_arity(args, 2)?;
        (self)(A::from_script_arg(&args[0])?, B::from_script_arg(&args[1])?).into_native_return()
    }
}

impl<F, A, B, C, R> TypedNativeFunction<(A, B, C)> for F
where
    F: Fn(A, B, C) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call(&self, args: &[Value]) -> VmResult<Value> {
        expect_arity(args, 3)?;
        (self)(
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
        )
        .into_native_return()
    }
}

impl<F, A, B, C, D, R> TypedNativeFunction<(A, B, C, D)> for F
where
    F: Fn(A, B, C, D) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    D: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call(&self, args: &[Value]) -> VmResult<Value> {
        expect_arity(args, 4)?;
        (self)(
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
            D::from_script_arg(&args[3])?,
        )
        .into_native_return()
    }
}
