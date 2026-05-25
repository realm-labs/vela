use vela_host::HostPath;
use vela_vm::{HostExecution, Value, VmResult};

use crate::FromScriptArg;

use super::{IntoNativeReturn, TypedNativeMethodFunction, expect_arity};

impl<F, R> TypedNativeMethodFunction<()> for F
where
    F: for<'host> Fn(&HostPath, &mut HostExecution<'host>) -> R + Send + Sync + 'static,
    R: IntoNativeReturn,
{
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        expect_arity(args, 0)?;
        (self)(receiver, host).into_native_return()
    }
}

impl<F, A, R> TypedNativeMethodFunction<(A,)> for F
where
    F: for<'host> Fn(&HostPath, &mut HostExecution<'host>, A) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        expect_arity(args, 1)?;
        (self)(receiver, host, A::from_script_arg(&args[0])?).into_native_return()
    }
}

impl<F, A, B, R> TypedNativeMethodFunction<(A, B)> for F
where
    F: for<'host> Fn(&HostPath, &mut HostExecution<'host>, A, B) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        expect_arity(args, 2)?;
        (self)(
            receiver,
            host,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
        )
        .into_native_return()
    }
}

impl<F, A, B, C, R> TypedNativeMethodFunction<(A, B, C)> for F
where
    F: for<'host> Fn(&HostPath, &mut HostExecution<'host>, A, B, C) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        expect_arity(args, 3)?;
        (self)(
            receiver,
            host,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
        )
        .into_native_return()
    }
}

impl<F, A, B, C, D, R> TypedNativeMethodFunction<(A, B, C, D)> for F
where
    F: for<'host> Fn(&HostPath, &mut HostExecution<'host>, A, B, C, D) -> R + Send + Sync + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    D: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        expect_arity(args, 4)?;
        (self)(
            receiver,
            host,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
            D::from_script_arg(&args[3])?,
        )
        .into_native_return()
    }
}

impl<F, A, B, C, D, E, R> TypedNativeMethodFunction<(A, B, C, D, E)> for F
where
    F: for<'host> Fn(&HostPath, &mut HostExecution<'host>, A, B, C, D, E) -> R
        + Send
        + Sync
        + 'static,
    A: FromScriptArg,
    B: FromScriptArg,
    C: FromScriptArg,
    D: FromScriptArg,
    E: FromScriptArg,
    R: IntoNativeReturn,
{
    fn call_method(
        &self,
        receiver: &HostPath,
        args: &[Value],
        host: &mut HostExecution<'_>,
    ) -> VmResult<Value> {
        expect_arity(args, 5)?;
        (self)(
            receiver,
            host,
            A::from_script_arg(&args[0])?,
            B::from_script_arg(&args[1])?,
            C::from_script_arg(&args[2])?,
            D::from_script_arg(&args[3])?,
            E::from_script_arg(&args[4])?,
        )
        .into_native_return()
    }
}
