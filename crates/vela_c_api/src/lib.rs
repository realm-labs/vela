//! C ABI entrypoints for embedding Vela from non-Rust hosts.
//!
//! This crate is intentionally separate from `vela_hot_reload`: hot-reload ABI
//! describes script compatibility, while this crate owns the external binary
//! interface. The first slice exposes opaque engine/runtime handles and scalar
//! function results. Host object vtables and aggregate value handles should be
//! added here without leaking Rust references across the ABI boundary.

use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{mem, ptr, slice};

use vela_common::ScalarValue;
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
    I8 = 3,
    I16 = 4,
    I32 = 5,
    I64 = 6,
    U8 = 7,
    U16 = 8,
    U32 = 9,
    U64 = 10,
    F32 = 11,
    F64 = 12,
    String = 13,
    Bytes = 14,
    Char = 15,
}

#[repr(C)]
#[derive(Debug)]
pub struct VelaCValue {
    pub kind: VelaCValueKind,
    pub bool_value: u8,
    pub i8_value: i8,
    pub i16_value: i16,
    pub i32_value: i32,
    pub i64_value: i64,
    pub u8_value: u8,
    pub u16_value: u16,
    pub u32_value: u32,
    pub u64_value: u64,
    pub f32_value: f32,
    pub f64_value: f64,
    pub string_value: *mut c_char,
    pub bytes_data: *mut u8,
    pub bytes_len: usize,
    pub char_value: u32,
}

impl VelaCValue {
    const fn missing() -> Self {
        Self {
            kind: VelaCValueKind::Missing,
            bool_value: 0,
            i8_value: 0,
            i16_value: 0,
            i32_value: 0,
            i64_value: 0,
            u8_value: 0,
            u16_value: 0,
            u32_value: 0,
            u64_value: 0,
            f32_value: 0.0,
            f64_value: 0.0,
            string_value: ptr::null_mut(),
            bytes_data: ptr::null_mut(),
            bytes_len: 0,
            char_value: 0,
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

/// Frees a byte buffer allocated by Vela C ABI functions.
///
/// # Safety
///
/// `data` must be either null with `len == 0`, or a pointer/length pair
/// returned by this crate for a bytes value. It must not be freed more than
/// once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_bytes_free(data: *mut u8, len: usize) {
    if data.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(ptr::slice_from_raw_parts_mut(data, len)));
    }
}

/// Releases resources held by a `VelaCValue`.
///
/// # Safety
///
/// `value` must be either null or a valid mutable pointer to a `VelaCValue`
/// initialized by the caller or returned through this ABI. If it contains a
/// string or bytes buffer, the pointer must still be owned by that value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_value_free(value: *mut VelaCValue) {
    if value.is_null() {
        return;
    }
    unsafe {
        let value = &mut *value;
        match value.kind {
            VelaCValueKind::String => vela_string_free(value.string_value),
            VelaCValueKind::Bytes => vela_bytes_free(value.bytes_data, value.bytes_len),
            _ => {}
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
            .compile_source(source)
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
        let c_value = call_runtime(runtime, entry, CallArgs::new())?;
        unsafe {
            *result_out = c_value;
        }
        Ok(())
    })
}

/// Calls a script entry with positional value arguments.
///
/// # Safety
///
/// `runtime` must be a valid pointer returned by `vela_runtime_compile_source`.
/// `entry` must be a valid null-terminated UTF-8 string. If `arg_count > 0`,
/// `args` must point to `arg_count` readable `VelaCValue` entries. The call
/// borrows input strings and bytes only for the duration of the call and copies
/// them into Vela-owned values. `result_out` and `error_out` follow the same
/// ownership rules as `vela_runtime_call`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vela_runtime_call_with_args(
    runtime: *mut VelaRuntime,
    entry: *const c_char,
    args: *const VelaCValue,
    arg_count: usize,
    result_out: *mut VelaCValue,
    error_out: *mut *mut c_char,
) -> VelaStatus {
    ffi_status(error_out, || {
        if result_out.is_null() {
            return Err((VelaStatus::NullPointer, "result pointer is null".to_owned()));
        }
        let runtime = non_null_mut(runtime, "runtime")?;
        let entry = c_str(entry, "entry")?;
        let args = c_args(args, arg_count)?;
        let c_value = call_runtime(runtime, entry, CallArgs::from_positional(args))?;
        unsafe {
            *result_out = c_value;
        }
        Ok(())
    })
}

fn call_runtime(
    runtime: &mut VelaRuntime,
    entry: &str,
    args: CallArgs<'_>,
) -> Result<VelaCValue, (VelaStatus, String)> {
    let value = runtime
        .runtime
        .call(entry, args, CallOptions::unbounded())
        .map_err(|error| (VelaStatus::RuntimeError, error.to_string()))?;
    let owned = runtime
        .runtime
        .value_to_owned(&value)
        .map_err(|error| (VelaStatus::RuntimeError, error.to_string()))?;
    value_to_c(owned)
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
        OwnedValue::Char(value) => Ok(VelaCValue {
            kind: VelaCValueKind::Char,
            char_value: value as u32,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::I8(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::I8,
            i8_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::I16(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::I16,
            i16_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::I32(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::I32,
            i32_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::I64(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::I64,
            i64_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::U8(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::U8,
            u8_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::U16(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::U16,
            u16_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::U32(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::U32,
            u32_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::U64(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::U64,
            u64_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::F32(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::F32,
            f32_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::Scalar(ScalarValue::F64(value)) => Ok(VelaCValue {
            kind: VelaCValueKind::F64,
            f64_value: value,
            ..VelaCValue::missing()
        }),
        OwnedValue::String(value) => Ok(VelaCValue {
            kind: VelaCValueKind::String,
            string_value: c_string_ptr(value),
            ..VelaCValue::missing()
        }),
        OwnedValue::Bytes(value) => {
            let (bytes_data, bytes_len) = c_bytes_ptr(value);
            Ok(VelaCValue {
                kind: VelaCValueKind::Bytes,
                bytes_data,
                bytes_len,
                ..VelaCValue::missing()
            })
        }
        _ => Err((
            VelaStatus::UnsupportedValue,
            "C ABI scalar call result does not support aggregate or host values yet".to_owned(),
        )),
    }
}

fn c_args(
    args: *const VelaCValue,
    arg_count: usize,
) -> Result<Vec<OwnedValue>, (VelaStatus, String)> {
    if arg_count == 0 {
        return Ok(Vec::new());
    }
    if args.is_null() {
        return Err((VelaStatus::NullPointer, "args pointer is null".to_owned()));
    }
    unsafe { slice::from_raw_parts(args, arg_count) }
        .iter()
        .map(c_value_to_owned)
        .collect()
}

fn c_value_to_owned(value: &VelaCValue) -> Result<OwnedValue, (VelaStatus, String)> {
    match value.kind {
        VelaCValueKind::Missing => Ok(OwnedValue::Missing),
        VelaCValueKind::Null => Ok(OwnedValue::Null),
        VelaCValueKind::Bool => Ok(OwnedValue::Bool(value.bool_value != 0)),
        VelaCValueKind::Char => {
            let Some(value) = char::from_u32(value.char_value) else {
                return Err((
                    VelaStatus::UnsupportedValue,
                    "char value is not a Unicode scalar".to_owned(),
                ));
            };
            Ok(OwnedValue::Char(value))
        }
        VelaCValueKind::I8 => Ok(OwnedValue::Scalar(ScalarValue::I8(value.i8_value))),
        VelaCValueKind::I16 => Ok(OwnedValue::Scalar(ScalarValue::I16(value.i16_value))),
        VelaCValueKind::I32 => Ok(OwnedValue::Scalar(ScalarValue::I32(value.i32_value))),
        VelaCValueKind::I64 => Ok(OwnedValue::Scalar(ScalarValue::I64(value.i64_value))),
        VelaCValueKind::U8 => Ok(OwnedValue::Scalar(ScalarValue::U8(value.u8_value))),
        VelaCValueKind::U16 => Ok(OwnedValue::Scalar(ScalarValue::U16(value.u16_value))),
        VelaCValueKind::U32 => Ok(OwnedValue::Scalar(ScalarValue::U32(value.u32_value))),
        VelaCValueKind::U64 => Ok(OwnedValue::Scalar(ScalarValue::U64(value.u64_value))),
        VelaCValueKind::F32 => Ok(OwnedValue::Scalar(ScalarValue::F32(value.f32_value))),
        VelaCValueKind::F64 => Ok(OwnedValue::Scalar(ScalarValue::F64(value.f64_value))),
        VelaCValueKind::String => {
            let value = c_str(value.string_value, "string value")?;
            Ok(OwnedValue::String(value.to_owned()))
        }
        VelaCValueKind::Bytes => {
            if value.bytes_len == 0 {
                return Ok(OwnedValue::Bytes(Vec::new()));
            }
            if value.bytes_data.is_null() {
                return Err((VelaStatus::NullPointer, "bytes pointer is null".to_owned()));
            }
            let bytes = unsafe { slice::from_raw_parts(value.bytes_data, value.bytes_len) };
            Ok(OwnedValue::Bytes(bytes.to_vec()))
        }
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

fn c_bytes_ptr(value: Vec<u8>) -> (*mut u8, usize) {
    if value.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let mut value = value.into_boxed_slice();
    let len = value.len();
    let data = value.as_mut_ptr();
    mem::forget(value);
    (data, len)
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

    fn compile_runtime(source: &str) -> (*mut VelaEngine, *mut VelaRuntime, *mut c_char) {
        let mut error = ptr::null_mut();
        let engine = unsafe { vela_engine_new(&mut error) };
        assert!(!engine.is_null());
        assert!(error.is_null());

        let source = c_string(source);
        let runtime = unsafe { vela_runtime_compile_source(engine, source.as_ptr(), &mut error) };
        assert!(!runtime.is_null());
        assert!(error.is_null());
        (engine, runtime, error)
    }

    fn input_i32(value: i32) -> VelaCValue {
        VelaCValue {
            kind: VelaCValueKind::I32,
            i32_value: value,
            ..VelaCValue::default()
        }
    }

    fn input_u32(value: u32) -> VelaCValue {
        VelaCValue {
            kind: VelaCValueKind::U32,
            u32_value: value,
            ..VelaCValue::default()
        }
    }

    fn input_f32(value: f32) -> VelaCValue {
        VelaCValue {
            kind: VelaCValueKind::F32,
            f32_value: value,
            ..VelaCValue::default()
        }
    }

    fn input_f64(value: f64) -> VelaCValue {
        VelaCValue {
            kind: VelaCValueKind::F64,
            f64_value: value,
            ..VelaCValue::default()
        }
    }

    fn input_char(value: char) -> VelaCValue {
        VelaCValue {
            kind: VelaCValueKind::Char,
            char_value: value as u32,
            ..VelaCValue::default()
        }
    }

    fn input_bytes(bytes: &mut [u8]) -> VelaCValue {
        VelaCValue {
            kind: VelaCValueKind::Bytes,
            bytes_data: bytes.as_mut_ptr(),
            bytes_len: bytes.len(),
            ..VelaCValue::default()
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
        assert_eq!(result.kind, VelaCValueKind::I64);
        assert_eq!(result.i64_value, 42);

        unsafe {
            vela_value_free(&mut result);
            vela_runtime_free(runtime);
            vela_engine_free(engine);
        }
    }

    #[test]
    fn c_api_returns_exact_scalar_and_bytes_tags() {
        let (engine, runtime, mut error) = compile_runtime(
            r#"
fn i8_value() -> i8 { return -8i8; }
fn i16_value() -> i16 { return -16i16; }
fn i32_value() -> i32 { return -32i32; }
fn i64_value() -> i64 { return -64i64; }
fn u8_value() -> u8 { return 8u8; }
fn u16_value() -> u16 { return 16u16; }
fn u32_value() -> u32 { return 32u32; }
fn u64_value() -> u64 { return 64u64; }
fn f32_value() -> f32 { return 1.5f32; }
fn f64_value() -> f64 { return 2.5f64; }
fn char_value() -> char { return '奖'; }
fn bytes_value() -> Bytes { return b"\x00\xff"; }
"#,
        );

        let mut result = VelaCValue::default();

        let entry = c_string("i8_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::I8);
        assert_eq!(result.i8_value, -8);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("i16_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::I16);
        assert_eq!(result.i16_value, -16);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("i32_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::I32);
        assert_eq!(result.i32_value, -32);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("i64_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::I64);
        assert_eq!(result.i64_value, -64);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("u8_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::U8);
        assert_eq!(result.u8_value, 8);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("u16_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::U16);
        assert_eq!(result.u16_value, 16);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("u32_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::U32);
        assert_eq!(result.u32_value, 32);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("u64_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::U64);
        assert_eq!(result.u64_value, 64);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("f32_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::F32);
        assert_eq!(result.f32_value, 1.5);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("f64_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::F64);
        assert_eq!(result.f64_value, 2.5);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("char_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::Char);
        assert_eq!(char::from_u32(result.char_value), Some('奖'));
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("bytes_value");
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::Bytes);
        assert_eq!(result.bytes_len, 2);
        assert!(!result.bytes_data.is_null());
        let bytes = unsafe { slice::from_raw_parts(result.bytes_data, result.bytes_len) };
        assert_eq!(bytes, &[0, 255]);

        unsafe {
            vela_value_free(&mut result);
            vela_runtime_free(runtime);
            vela_engine_free(engine);
        }
    }

    #[test]
    fn c_api_call_with_args_passes_scalars_and_bytes() {
        let (engine, runtime, mut error) = compile_runtime(
            r#"
fn id_i32(value: i32) -> i32 { return value; }
fn id_u32(value: u32) -> u32 { return value; }
fn id_f32(value: f32) -> f32 { return value; }
fn id_f64(value: f64) -> f64 { return value; }
fn id_char(value: char) -> char { return value; }
fn id_bytes(value: Bytes) -> Bytes { return value; }
"#,
        );

        let mut result = VelaCValue::default();

        let entry = c_string("id_i32");
        let args = [input_i32(-123)];
        let status = unsafe {
            vela_runtime_call_with_args(
                runtime,
                entry.as_ptr(),
                args.as_ptr(),
                args.len(),
                &mut result,
                &mut error,
            )
        };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::I32);
        assert_eq!(result.i32_value, -123);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("id_u32");
        let args = [input_u32(123)];
        let status = unsafe {
            vela_runtime_call_with_args(
                runtime,
                entry.as_ptr(),
                args.as_ptr(),
                args.len(),
                &mut result,
                &mut error,
            )
        };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::U32);
        assert_eq!(result.u32_value, 123);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("id_f32");
        let args = [input_f32(1.25)];
        let status = unsafe {
            vela_runtime_call_with_args(
                runtime,
                entry.as_ptr(),
                args.as_ptr(),
                args.len(),
                &mut result,
                &mut error,
            )
        };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::F32);
        assert_eq!(result.f32_value, 1.25);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("id_f64");
        let args = [input_f64(2.5)];
        let status = unsafe {
            vela_runtime_call_with_args(
                runtime,
                entry.as_ptr(),
                args.as_ptr(),
                args.len(),
                &mut result,
                &mut error,
            )
        };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::F64);
        assert_eq!(result.f64_value, 2.5);
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("id_char");
        let args = [input_char('励')];
        let status = unsafe {
            vela_runtime_call_with_args(
                runtime,
                entry.as_ptr(),
                args.as_ptr(),
                args.len(),
                &mut result,
                &mut error,
            )
        };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::Char);
        assert_eq!(char::from_u32(result.char_value), Some('励'));
        unsafe { vela_value_free(&mut result) };

        let entry = c_string("id_bytes");
        let mut bytes = [0, 1, 255];
        let args = [input_bytes(&mut bytes)];
        let status = unsafe {
            vela_runtime_call_with_args(
                runtime,
                entry.as_ptr(),
                args.as_ptr(),
                args.len(),
                &mut result,
                &mut error,
            )
        };
        assert_eq!(status, VelaStatus::Ok);
        assert_eq!(result.kind, VelaCValueKind::Bytes);
        assert_eq!(result.bytes_len, 3);
        let returned = unsafe { slice::from_raw_parts(result.bytes_data, result.bytes_len) };
        assert_eq!(returned, &[0, 1, 255]);

        unsafe {
            vela_value_free(&mut result);
            vela_runtime_free(runtime);
            vela_engine_free(engine);
        }
    }

    #[test]
    fn c_api_reports_runtime_contract_errors() {
        let mut error = ptr::null_mut();
        let engine = unsafe { vela_engine_new(&mut error) };
        assert!(!engine.is_null());
        assert!(error.is_null());

        let source = c_string(
            r#"
fn dynamic() {
    return "bad";
}

fn main() -> i64 {
    return dynamic();
}
"#,
        );
        let runtime = unsafe { vela_runtime_compile_source(engine, source.as_ptr(), &mut error) };
        assert!(!runtime.is_null());
        assert!(error.is_null());

        let entry = c_string("main");
        let mut result = VelaCValue::default();
        let status = unsafe { vela_runtime_call(runtime, entry.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, VelaStatus::RuntimeError);
        assert!(!error.is_null());

        unsafe {
            vela_string_free(error);
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
