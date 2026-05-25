use vela_vm::{Value, VmError, VmErrorKind, VmResult};

use crate::{FromScriptArg, IntoScriptArg, NativeCallContext};

pub trait IntoNativeReturn {
    fn into_native_return(self) -> VmResult<Value>;
}

impl<T> IntoNativeReturn for T
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<Value> {
        Ok(self.into_script_arg())
    }
}

impl<T> IntoNativeReturn for VmResult<T>
where
    T: IntoScriptArg,
{
    fn into_native_return(self) -> VmResult<Value> {
        self.map(IntoScriptArg::into_script_arg)
    }
}

pub trait TypedNativeFunction<Args>: Send + Sync + 'static {
    fn call(&self, args: &[Value]) -> VmResult<Value>;
}

pub trait TypedContextHostNativeFunction<Args>: Send + Sync + 'static {
    fn call_context(&self, args: &[Value], ctx: &mut NativeCallContext<'_, '_>) -> VmResult<Value>;
}

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

fn expect_arity(args: &[Value], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError {
        kind: VmErrorKind::ArityMismatch {
            name: "typed native function".to_owned(),
            expected,
            actual: args.len(),
        },
        source_span: None,
    })
}
