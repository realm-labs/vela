use std::io::Write;
use std::path::{Component, Path, PathBuf};

use vela_common::stable_id;
use vela_def::FunctionId;
use vela_reflect::modules::ModuleDesc;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId, TypeHint,
};

pub const IO_PRINTLN_FUNCTION_ID: NativeFunctionId =
    FunctionId::new(stable_id("std_function", "io", "println") as u128);
pub const FS_READ_TO_STRING_FUNCTION_ID: NativeFunctionId =
    FunctionId::new(stable_id("std_function", "fs", "read_to_string") as u128);
pub const FS_WRITE_STRING_FUNCTION_ID: NativeFunctionId =
    FunctionId::new(stable_id("std_function", "fs", "write_string") as u128);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsSandbox {
    root: PathBuf,
}

impl FsSandbox {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve_read(&self, path: &str) -> Result<PathBuf, OwnedValue> {
        let relative = sanitize_relative_path(path)?;
        let root = canonical_root(&self.root)?;
        let resolved = root
            .join(relative)
            .canonicalize()
            .map_err(|error| io_error("read", path, format!("failed to resolve path: {error}")))?;
        if !resolved.starts_with(&root) {
            return Err(io_error("sandbox", path, "path escapes fs sandbox"));
        }
        Ok(resolved)
    }

    fn resolve_write(&self, path: &str) -> Result<PathBuf, OwnedValue> {
        let relative = sanitize_relative_path(path)?;
        let root = canonical_root(&self.root)?;
        let resolved = root.join(relative);
        let parent = resolved.parent().unwrap_or(&root);
        let parent = parent.canonicalize().map_err(|error| {
            io_error(
                "write",
                path,
                format!("failed to resolve parent directory: {error}"),
            )
        })?;
        if !parent.starts_with(&root) {
            return Err(io_error("sandbox", path, "path escapes fs sandbox"));
        }
        Ok(resolved)
    }
}

pub(crate) fn io_module_desc() -> ModuleDesc {
    ModuleDesc::new("io")
        .docs("Opt-in standard output helpers.")
        .attr("stdlib", "io")
        .attr("domain", "io")
}

pub(crate) fn fs_module_desc() -> ModuleDesc {
    ModuleDesc::new("fs")
        .docs("Opt-in sandboxed filesystem helpers.")
        .attr("stdlib", "fs")
        .attr("domain", "io")
}

pub(crate) fn stdio_functions() -> [NativeFunctionEntry; 1] {
    [NativeFunctionEntry::new(
        NativeFunctionDesc::new("io::println", IO_PRINTLN_FUNCTION_ID)
            .param("value", TypeHint::Any)
            .returns(TypeHint::Any)
            .effects(EffectSet::io_write())
            .access(FunctionAccess::public().reflect_callable(true))
            .attr("stdlib", "io")
            .attr("domain", "io")
            .docs("Writes a formatted value and newline to stdout."),
        io_println,
    )]
}

pub(crate) fn fs_functions(sandbox: FsSandbox) -> [NativeFunctionEntry; 2] {
    let read_sandbox = sandbox.clone();
    let write_sandbox = sandbox;
    [
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("fs::read_to_string", FS_READ_TO_STRING_FUNCTION_ID)
                .param("path", TypeHint::String)
                .returns(TypeHint::Any)
                .effects(EffectSet::io_read())
                .access(FunctionAccess::public().reflect_callable(true))
                .attr("stdlib", "fs")
                .attr("domain", "io")
                .docs("Reads a UTF-8 file under the configured filesystem sandbox."),
            move |args| fs_read_to_string(args, &read_sandbox),
        ),
        NativeFunctionEntry::new(
            NativeFunctionDesc::new("fs::write_string", FS_WRITE_STRING_FUNCTION_ID)
                .param("path", TypeHint::String)
                .param("text", TypeHint::String)
                .returns(TypeHint::Any)
                .effects(EffectSet::io_write())
                .access(FunctionAccess::public().reflect_callable(true))
                .attr("stdlib", "fs")
                .attr("domain", "io")
                .docs("Writes a UTF-8 file under the configured filesystem sandbox."),
            move |args| fs_write_string(args, &write_sandbox),
        ),
    ]
}

fn io_println(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("io::println", args, 1)?;
    let mut stdout = std::io::stdout().lock();
    match writeln!(stdout, "{}", display_value(&args[0])) {
        Ok(()) => Ok(result_ok(OwnedValue::Null)),
        Err(error) => Ok(result_err(io_error("write", "stdout", error.to_string()))),
    }
}

fn fs_read_to_string(args: &[OwnedValue], sandbox: &FsSandbox) -> VmResult<OwnedValue> {
    expect_arity("fs::read_to_string", args, 1)?;
    let path = expect_string("fs::read_to_string", &args[0])?;
    match sandbox.resolve_read(path) {
        Ok(resolved) => match std::fs::read_to_string(&resolved) {
            Ok(text) => Ok(result_ok(text)),
            Err(error) => Ok(result_err(io_error("read", path, error.to_string()))),
        },
        Err(error) => Ok(result_err(error)),
    }
}

fn fs_write_string(args: &[OwnedValue], sandbox: &FsSandbox) -> VmResult<OwnedValue> {
    expect_arity("fs::write_string", args, 2)?;
    let path = expect_string("fs::write_string", &args[0])?;
    let text = expect_string("fs::write_string", &args[1])?;
    match sandbox.resolve_write(path) {
        Ok(resolved) => match std::fs::write(&resolved, text) {
            Ok(()) => Ok(result_ok(OwnedValue::Null)),
            Err(error) => Ok(result_err(io_error("write", path, error.to_string()))),
        },
        Err(error) => Ok(result_err(error)),
    }
}

fn sanitize_relative_path(path: &str) -> Result<PathBuf, OwnedValue> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err(io_error("path", "", "path must not be empty"));
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(io_error(
                    "sandbox",
                    path.to_string_lossy(),
                    "only relative paths inside the fs sandbox are allowed",
                ));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(io_error(
            "path",
            path.to_string_lossy(),
            "path must name a file",
        ));
    }
    Ok(relative)
}

fn canonical_root(root: &Path) -> Result<PathBuf, OwnedValue> {
    root.canonicalize().map_err(|error| {
        io_error(
            "sandbox",
            root.to_string_lossy(),
            format!("failed to resolve fs sandbox root: {error}"),
        )
    })
}

fn result_ok(value: impl Into<OwnedValue>) -> OwnedValue {
    OwnedValue::enum_variant("Result", "Ok", [("0", value.into())])
}

fn result_err(value: impl Into<OwnedValue>) -> OwnedValue {
    OwnedValue::enum_variant("Result", "Err", [("0", value.into())])
}

fn io_error(
    kind: impl Into<String>,
    path: impl AsRef<str>,
    message: impl Into<String>,
) -> OwnedValue {
    OwnedValue::record(
        "IoError",
        [
            ("kind", OwnedValue::String(kind.into())),
            ("path", OwnedValue::String(path.as_ref().to_owned())),
            ("message", OwnedValue::String(message.into())),
        ],
    )
}

fn display_value(value: &OwnedValue) -> String {
    match value {
        OwnedValue::Missing => "<missing>".to_owned(),
        OwnedValue::Null => "null".to_owned(),
        OwnedValue::Bool(value) => value.to_string(),
        OwnedValue::Int(value) => value.to_string(),
        OwnedValue::Float(value) => value.to_string(),
        OwnedValue::String(value) => value.clone(),
        OwnedValue::Array(values) => {
            let values = values.iter().map(display_value).collect::<Vec<_>>();
            format!("[{}]", values.join(", "))
        }
        OwnedValue::Map(entries) => {
            let entries = entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", display_value(value)))
                .collect::<Vec<_>>();
            format!("{{{}}}", entries.join(", "))
        }
        OwnedValue::Set(values) => {
            let values = values.iter().map(display_value).collect::<Vec<_>>();
            format!("{{{}}}", values.join(", "))
        }
        OwnedValue::Record { type_name, fields } => {
            let fields = fields
                .iter()
                .map(|(field, value)| format!("{field}: {}", display_value(value)))
                .collect::<Vec<_>>();
            format!("{type_name}{{{}}}", fields.join(", "))
        }
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => {
            let fields = fields
                .iter()
                .map(|(field, value)| format!("{field}: {}", display_value(value)))
                .collect::<Vec<_>>();
            format!("{enum_name}::{variant}({})", fields.join(", "))
        }
        OwnedValue::Closure(_) => "<closure>".to_owned(),
        OwnedValue::Range(value) => format!("{value:?}"),
        OwnedValue::HostRef(value) => format!("{value:?}"),
        OwnedValue::PathProxy(value) => format!("{value:?}"),
        OwnedValue::Iterator(_) => "<iterator>".to_owned(),
    }
}

fn expect_string<'a>(operation: &'static str, value: &'a OwnedValue) -> VmResult<&'a str> {
    if let OwnedValue::String(value) = value {
        return Ok(value);
    }
    type_error(operation)
}

fn expect_arity(name: &str, args: &[OwnedValue], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
