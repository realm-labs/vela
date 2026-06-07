//! C ABI entrypoints for embedding Vela from non-Rust hosts.
//!
//! This crate is intentionally separate from `vela_hot_reload`: hot-reload ABI
//! describes script compatibility, while this crate owns the external binary
//! interface. The first slice exposes opaque engine/runtime handles and scalar
//! function results. Host object vtables and aggregate value handles should be
//! added here without leaking Rust references across the ABI boundary.

use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use vela_common::SourceId;
use vela_engine::engine::Engine;
use vela_engine::permission::ExecutionProfile;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_vm::owned_value::OwnedValue;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VelaApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VelaStatus {
    Ok = 0,
    NullPointer = 1,
    InvalidUtf8 = 2,
    EngineError = 3,
    CompileError = 4,
    RuntimeError = 5,
    UnsupportedValue = 6,
    Panic = 7,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VelaCValueKind {
    Missing = 0,
    Null = 1,
    Bool = 2,
    Int = 3,
    Float = 4,
    String = 5,
}

#[repr(C)]
#[derive(Debug)]
pub struct VelaCValue {
    pub kind: VelaCValueKind,
    pub bool_value: u8,
    pub int_value: i64,
    pub float_value: f64,
    pub string_value: *mut c_char,
}

impl VelaCValue {
    const fn missing() -> Self {
        Self {
            kind: VelaCValueKind::Missing,
            bool_value: 0,
            int_value: 0,
            float_value: 0.0,
            string_value: ptr::null_mut(),
        }
    }
}

impl Default for VelaCValue {
    fn default() -> Self {
        Self::missing()
    }
}

pub struct VelaEngine {
    engine: Engine,
}

pub struct VelaRuntime {
    runtime: Runtime,
}

#[unsafe(no_mangle)]
pub extern "C" fn vela_api_version() -> VelaApiVersion {
    VelaApiVersion {
        major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0),
        minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0),
        patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0),
    }
}

/// Frees a string allocated by Vela C ABI functions.
///
/// # Safety
///
/// `value` must be either null or a pointer previously returned by this crate
/// through `CString::into_raw`, and it must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(value));
    }
}

/// Releases resources held by a `VelaCValue`.
///
/// # Safety
///
/// `value` must be either null or a valid mutable pointer to a `VelaCValue`
/// initialized by the caller or returned through this ABI. If it contains a
/// string, the string pointer must still be owned by that value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_value_free(value: *mut VelaCValue) {
    if value.is_null() {
        return;
    }
    unsafe {
        let value = &mut *value;
        if value.kind == VelaCValueKind::String {
            vela_string_free(value.string_value);
        }
        *value = VelaCValue::missing();
    }
}

/// Creates a default trusted Vela engine with standard natives installed.
///
/// # Safety
///
/// `error_out` may be null. If non-null, it must point to writable storage for
/// one `char*`; any non-null error string written there must be freed with
/// `vela_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_engine_new(error_out: *mut *mut c_char) -> *mut VelaEngine {
    ffi_ptr(error_out, || {
        let engine = Engine::builder()
            .with_standard_natives()
            .execution_profile(ExecutionProfile::trusted())
            .build()
            .map_err(|error| (VelaStatus::EngineError, error.to_string()))?;
        Ok(Box::into_raw(Box::new(VelaEngine { engine })))
    })
}

/// Frees an engine created by `vela_engine_new`.
///
/// # Safety
///
/// `engine` must be either null or a pointer returned by `vela_engine_new`, and
/// it must not be used or freed again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_engine_free(engine: *mut VelaEngine) {
    if engine.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(engine));
    }
}

/// Compiles a UTF-8 source string into a runtime.
///
/// # Safety
///
/// `engine` must be a valid pointer returned by `vela_engine_new`. `source`
/// must be a valid null-terminated UTF-8 string. `error_out` follows the same
/// ownership rules as `vela_engine_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_runtime_compile_source(
    engine: *const VelaEngine,
    source: *const c_char,
    error_out: *mut *mut c_char,
) -> *mut VelaRuntime {
    ffi_ptr(error_out, || {
        let engine = non_null_ref(engine, "engine")?;
        let source = c_str(source, "source")?;
        let program = engine
            .engine
            .compile_source(SourceId::new(1), source)
            .map_err(|error| (VelaStatus::CompileError, error.to_string()))?;
        let runtime = Runtime::new(engine.engine.clone(), program);
        Ok(Box::into_raw(Box::new(VelaRuntime { runtime })))
    })
}

/// Frees a runtime created by `vela_runtime_compile_source`.
///
/// # Safety
///
/// `runtime` must be either null or a pointer returned by
/// `vela_runtime_compile_source`, and it must not be used or freed again after
/// this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_runtime_free(runtime: *mut VelaRuntime) {
    if runtime.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(runtime));
    }
}

/// Calls a no-argument script entry and writes a scalar result.
///
/// # Safety
///
/// `runtime` must be a valid pointer returned by `vela_runtime_compile_source`.
/// `entry` must be a valid null-terminated UTF-8 string. `result_out` must be
/// a valid mutable pointer to writable `VelaCValue` storage. `error_out`
/// follows the same ownership rules as `vela_engine_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_runtime_call(
    runtime: *mut VelaRuntime,
    entry: *const c_char,
    result_out: *mut VelaCValue,
    error_out: *mut *mut c_char,
) -> VelaStatus {
    ffi_status(error_out, || {
        if result_out.is_null() {
            return Err((VelaStatus::NullPointer, "result pointer is null".to_owned()));
        }
        let runtime = non_null_mut(runtime, "runtime")?;
        let entry = c_str(entry, "entry")?;
        let value = runtime
            .runtime
            .call(entry, CallArgs::new(), CallOptions::unbounded())
            .map_err(|error| (VelaStatus::RuntimeError, error.to_string()))?;
        let owned = runtime
            .runtime
            .value_to_owned(&value)
            .map_err(|error| (VelaStatus::RuntimeError, error.to_string()))?;
        let c_value = value_to_c(owned)?;
        unsafe {
            *result_out = c_value;
        }
        Ok(())
    })
}

fn value_to_c(value: OwnedValue) -> Result<VelaCValue, (VelaStatus, String)> {
    match value {
        OwnedValue::Missing => Ok(VelaCValue::missing()),
        OwnedValue::Null => Ok(VelaCValue {
            kind: VelaCValueKind::Null,
            ..VelaCValue::missing()
        }),
        OwnedValue::Bool(value) => Ok(VelaCValue {
            kind: VelaCValueKind::Bool,
            bool_value: u8::from(value),
            ..VelaCValue::missing()
        }),
        OwnedValue::Int(value) => Ok(VelaCValue {
            kind: VelaCValueKind::Int,
            int_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Float(value) => Ok(VelaCValue {
            kind: VelaCValueKind::Float,
            float_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::String(value) => Ok(VelaCValue {
            kind: VelaCValueKind::String,
            string_value: c_string_ptr(value),
            ..VelaCValue::missing()
        }),
        _ => Err((
            VelaStatus::UnsupportedValue,
            "C ABI scalar call result does not support aggregate or host values yet".to_owned(),
        )),
    }
}

fn ffi_ptr<T>(
    error_out: *mut *mut c_char,
    f: impl FnOnce() -> Result<*mut T, (VelaStatus, String)>,
) -> *mut T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => value,
        Ok(Err((_, message))) => {
            set_error(error_out, message);
            ptr::null_mut()
        }
        Err(_) => {
            set_error(error_out, "panic crossed Vela C ABI boundary");
            ptr::null_mut()
        }
    }
}

fn ffi_status(
    error_out: *mut *mut c_char,
    f: impl FnOnce() -> Result<(), (VelaStatus, String)>,
) -> VelaStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => VelaStatus::Ok,
        Ok(Err((status, message))) => {
            set_error(error_out, message);
            status
        }
        Err(_) => {
            set_error(error_out, "panic crossed Vela C ABI boundary");
            VelaStatus::Panic
        }
    }
}

fn non_null_ref<'a, T>(ptr: *const T, name: &str) -> Result<&'a T, (VelaStatus, String)> {
    if ptr.is_null() {
        return Err((VelaStatus::NullPointer, format!("{name} pointer is null")));
    }
    unsafe { Ok(&*ptr) }
}

fn non_null_mut<'a, T>(ptr: *mut T, name: &str) -> Result<&'a mut T, (VelaStatus, String)> {
    if ptr.is_null() {
        return Err((VelaStatus::NullPointer, format!("{name} pointer is null")));
    }
    unsafe { Ok(&mut *ptr) }
}

fn c_str<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, (VelaStatus, String)> {
    if ptr.is_null() {
        return Err((VelaStatus::NullPointer, format!("{name} pointer is null")));
    }
    let value = unsafe { CStr::from_ptr(ptr) };
    value.to_str().map_err(|error| {
        (
            VelaStatus::InvalidUtf8,
            format!("{name} is not valid UTF-8: {error}"),
        )
    })
}

fn set_error(error_out: *mut *mut c_char, message: impl AsRef<str>) {
    if error_out.is_null() {
        return;
    }
    let pointer = c_string_ptr(message.as_ref().to_owned());
    unsafe {
        *error_out = pointer;
    }
}

fn c_string_ptr(value: String) -> *mut c_char {
    let value = value.replace('\0', "\\0");
    match CString::new(value) {
        Ok(value) => value.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::ptr;

    use super::*;

    fn c_string(value: &str) -> CString {
        match CString::new(value) {
            Ok(value) => value,
            Err(error) => panic!("test string contains nul byte: {error}"),
        }
    }

    #[test]
    fn c_api_compiles_and_calls_scalar_entry() {
        let mut error = ptr::null_mut();
        let engine = unsafe { vela_engine_new(&mut error) };
        assert!(!engine.is_null());
        assert!(error.is_null());

        let source = c_string("fn main() { return 41 + 1; }");
        let runtime = unsafe { vela_runtime_compile_source(engine, source.as_ptr(), &mut error) };
        assert!(!runtime.is_null());
        assert!(error.is_null());

        let entry = c_string("main");
        let mut result = VelaCValue::default();
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert!(error.is_null());
        assert_eq!(result.kind, VelaCValueKind::Int);
        assert_eq!(result.int_value, 42);

        unsafe {
            vela_value_free(&mut result);
            vela_runtime_free(runtime);
            vela_engine_free(engine);
        }
    }

    #[test]
    fn c_api_reports_compile_errors() {
        let mut error = ptr::null_mut();
        let engine = unsafe { vela_engine_new(&mut error) };
        assert!(!engine.is_null());

        let source = c_string("fn main() { return 1; } fn main() { return 2; }");
        let runtime = unsafe { vela_runtime_compile_source(engine, source.as_ptr(), &mut error) };
        assert!(runtime.is_null());
        assert!(!error.is_null());

        unsafe {
            vela_string_free(error);
            vela_engine_free(engine);
        }
    }
}
