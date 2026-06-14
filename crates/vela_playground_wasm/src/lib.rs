use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use vela_bytecode::compiler::error::{CompileError, CompileErrorKind};
use vela_common::{Diagnostic, Severity, Span};
use vela_engine::prelude::{
    CallArgs, CallOptions, Capability, EngineBuilder, EngineSourceError, EngineSourceErrorKind,
    Runtime,
};
use vela_vm::owned_value::OwnedValue;
use wasm_bindgen::prelude::*;

const DEFAULT_INSTRUCTION_BUDGET: u64 = 250_000;
const DEFAULT_MEMORY_BUDGET: usize = 8 * 1024 * 1024;
const DEFAULT_CALL_DEPTH: usize = 128;

#[derive(Serialize)]
struct PlaygroundResponse {
    ok: bool,
    value: Option<JsonValue>,
    diagnostics: Vec<PlaygroundDiagnostic>,
}

#[derive(Serialize)]
struct PlaygroundDiagnostic {
    severity: &'static str,
    code: Option<String>,
    message: String,
    span: Option<PlaygroundSpan>,
    labels: Vec<PlaygroundLabel>,
}

#[derive(Serialize)]
struct PlaygroundLabel {
    message: String,
    span: PlaygroundSpan,
}

#[derive(Serialize)]
struct PlaygroundSpan {
    source: u32,
    start: u32,
    end: u32,
}

#[wasm_bindgen]
pub fn compile_script(source: &str) -> String {
    response_to_json(match compile_and_link_program(source) {
        Ok(()) => PlaygroundResponse {
            ok: true,
            value: Some(json!({ "status": "compiled" })),
            diagnostics: Vec::new(),
        },
        Err(response) => response,
    })
}

#[wasm_bindgen]
pub fn run_script(source: &str, entry: &str) -> String {
    response_to_json(run_script_inner(source, entry))
}

fn run_script_inner(source: &str, entry: &str) -> PlaygroundResponse {
    let engine = match playground_engine() {
        Ok(engine) => engine,
        Err(error) => return source_error_response(error),
    };
    let program = match engine.compile_source(source) {
        Ok(program) => program,
        Err(error) => return source_error_response(error),
    };
    let mut runtime = match Runtime::try_new(engine, program) {
        Ok(runtime) => runtime,
        Err(error) => return single_error_response(format!("failed to link program: {error}")),
    };
    match runtime.call(
        entry,
        CallArgs::new(),
        CallOptions::new(
            DEFAULT_INSTRUCTION_BUDGET,
            DEFAULT_MEMORY_BUDGET,
            DEFAULT_CALL_DEPTH,
        ),
    ) {
        Ok(value) => match runtime.value_to_owned(&value) {
            Ok(value) => PlaygroundResponse {
                ok: true,
                value: Some(owned_value_to_json(&value)),
                diagnostics: Vec::new(),
            },
            Err(error) => diagnostic_response(error.to_diagnostic()),
        },
        Err(error) => diagnostic_response(error.to_diagnostic()),
    }
}

fn compile_and_link_program(source: &str) -> Result<(), PlaygroundResponse> {
    let engine = playground_engine().map_err(source_error_response)?;
    let program = engine
        .compile_source(source)
        .map_err(source_error_response)?;
    Runtime::try_new(engine, program)
        .map(drop)
        .map_err(link_error_response)
}

fn link_error_response(error: vela_bytecode::linker::LinkError) -> PlaygroundResponse {
    single_error_response(format!("failed to link program: {error}"))
}

fn playground_engine() -> Result<vela_engine::prelude::Engine, EngineSourceError> {
    EngineBuilder::new()
        .with_standard_natives()
        .with_time_clock(1_700_000_000, 1)
        .with_controlled_random(0x5eed)
        .capability(Capability::Time)
        .capability(Capability::Random)
        .build()
        .map_err(|error| EngineSourceError {
            kind: EngineSourceErrorKind::Io {
                path: "playground engine".to_owned(),
                message: error.to_string(),
            },
        })
}

fn source_error_response(error: EngineSourceError) -> PlaygroundResponse {
    match error.kind {
        EngineSourceErrorKind::Compile(error) => compile_error_response(error),
        EngineSourceErrorKind::Io { path, message } => {
            single_error_response(format!("failed to read source {path}: {message}"))
        }
        EngineSourceErrorKind::InvalidSourcePath { path } => {
            single_error_response(format!("invalid source path {path}"))
        }
        EngineSourceErrorKind::TooManySources { count } => {
            single_error_response(format!("too many source files: {count}"))
        }
    }
}

fn compile_error_response(error: CompileError) -> PlaygroundResponse {
    if let Some(diagnostic) = error.to_diagnostic() {
        return diagnostic_response(diagnostic);
    }

    match error.kind {
        CompileErrorKind::SyntaxDiagnostics(diagnostics)
        | CompileErrorKind::SemanticDiagnostics(diagnostics) => PlaygroundResponse {
            ok: false,
            value: None,
            diagnostics: diagnostics.into_iter().map(playground_diagnostic).collect(),
        },
        CompileErrorKind::FunctionNotFound(name) => {
            single_error_response(format!("function `{name}` was not found"))
        }
        CompileErrorKind::UnknownLocal(name) => {
            single_error_response(format!("unknown local `{name}`"))
        }
        CompileErrorKind::InvalidIntLiteral { literal, error } => {
            single_error_response(format!("invalid int literal `{literal}`: {error}"))
        }
        CompileErrorKind::InvalidFloatLiteral { literal, error } => {
            single_error_response(format!("invalid float literal `{literal}`: {error}"))
        }
        CompileErrorKind::RegisterOverflow => single_error_response("register overflow"),
        CompileErrorKind::BytecodeVerification(error) => {
            single_error_response(format!("bytecode verification failed: {error:?}"))
        }
        CompileErrorKind::UnsupportedSyntax(message) => {
            single_error_response(format!("unsupported syntax: {message}"))
        }
    }
}

fn diagnostic_response(diagnostic: Diagnostic) -> PlaygroundResponse {
    PlaygroundResponse {
        ok: false,
        value: None,
        diagnostics: vec![playground_diagnostic(diagnostic)],
    }
}

fn single_error_response(message: impl Into<String>) -> PlaygroundResponse {
    PlaygroundResponse {
        ok: false,
        value: None,
        diagnostics: vec![PlaygroundDiagnostic {
            severity: "error",
            code: None,
            message: message.into(),
            span: None,
            labels: Vec::new(),
        }],
    }
}

fn playground_diagnostic(diagnostic: Diagnostic) -> PlaygroundDiagnostic {
    PlaygroundDiagnostic {
        severity: match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            Severity::Help => "help",
        },
        code: diagnostic.code,
        message: diagnostic.message,
        span: diagnostic.span.map(playground_span),
        labels: diagnostic
            .labels
            .into_iter()
            .map(|label| PlaygroundLabel {
                message: label.message,
                span: playground_span(label.span),
            })
            .collect(),
    }
}

fn playground_span(span: Span) -> PlaygroundSpan {
    PlaygroundSpan {
        source: span.source.get(),
        start: span.start,
        end: span.end,
    }
}

fn owned_value_to_json(value: &OwnedValue) -> JsonValue {
    match value {
        OwnedValue::Missing => json!({ "kind": "missing" }),
        OwnedValue::Null => JsonValue::Null,
        OwnedValue::Bool(value) => JsonValue::Bool(*value),
        OwnedValue::Scalar(vela_common::ScalarValue::I8(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::I16(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::I32(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::U8(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::U16(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::U32(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::U64(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::F32(value)) => json!(value),
        OwnedValue::Scalar(vela_common::ScalarValue::F64(value)) => json!(value),
        OwnedValue::Char(value) => json!({ "kind": "char", "value": value.to_string() }),
        OwnedValue::String(value) => json!(value),
        OwnedValue::Bytes(value) => json!({ "kind": "bytes", "values": value }),
        OwnedValue::Array(values) => {
            JsonValue::Array(values.iter().map(owned_value_to_json).collect())
        }
        OwnedValue::Map(entries) => JsonValue::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), owned_value_to_json(value)))
                .collect(),
        ),
        OwnedValue::Set(values) => json!({
            "kind": "set",
            "values": values.iter().map(owned_value_to_json).collect::<Vec<_>>(),
        }),
        OwnedValue::Record { type_name, fields } => json!({
            "kind": "record",
            "type": type_name,
            "fields": JsonValue::Object(
                fields
                    .iter()
                    .map(|(field, value)| (field.to_owned(), owned_value_to_json(value)))
                    .collect(),
            ),
        }),
        OwnedValue::Enum {
            enum_name,
            variant,
            fields,
        } => json!({
            "kind": "enum",
            "type": enum_name,
            "variant": variant,
            "fields": JsonValue::Object(
                fields
                    .iter()
                    .map(|(field, value)| (field.to_owned(), owned_value_to_json(value)))
                    .collect(),
            ),
        }),
        OwnedValue::Closure(_) => json!({ "kind": "closure" }),
        OwnedValue::Range(value) => json!({ "kind": "range", "value": format!("{value:?}") }),
        OwnedValue::HostRef(value) => json!({ "kind": "host_ref", "value": format!("{value:?}") }),
        OwnedValue::PathProxy(value) => {
            json!({ "kind": "path_proxy", "value": format!("{value:?}") })
        }
        OwnedValue::Iterator(_) => json!({ "kind": "iterator" }),
    }
}

fn response_to_json(response: PlaygroundResponse) -> String {
    serde_json::to_string(&response).unwrap_or_else(|error| {
        json!({
            "ok": false,
            "diagnostics": [{ "severity": "error", "message": error.to_string() }],
        })
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value as JsonValue;

    use super::{compile_script, run_script};

    #[test]
    fn run_script_returns_json_value() {
        let response: JsonValue = serde_json::from_str(&run_script(
            r#"
            fn main() {
                return ["vela", 42];
            }
            "#,
            "main",
        ))
        .expect("valid playground response");

        assert_eq!(response["ok"], true);
        assert_eq!(response["value"][0], "vela");
        assert_eq!(response["value"][1], 42);
    }

    #[test]
    fn run_script_reports_runtime_error() {
        let response: JsonValue = serde_json::from_str(&run_script(
            r#"
            fn main() {
                return 1 / 0;
            }
            "#,
            "main",
        ))
        .expect("valid playground response");

        assert_eq!(response["ok"], false);
        assert_eq!(response["diagnostics"][0]["code"], "vm::division_by_zero");
    }

    #[test]
    fn compile_script_accepts_dynamic_record_field_access() {
        let response: JsonValue = serde_json::from_str(&compile_script(
            r#"
            struct Reward {
                enabled: bool,
            }

            fn read_enabled(reward) {
                return reward.enabled;
            }
            "#,
        ))
        .expect("valid playground response");

        assert_eq!(response["ok"], true, "{response:#}");
    }

    #[test]
    fn run_script_runs_level_reward_playground_example() {
        let response: JsonValue = serde_json::from_str(&run_script(
            r#"
            struct Reward {
                enabled: bool,
                base: int,
                multiplier: int,
            }

            fn score_reward(reward) {
                if !reward.enabled {
                    return 0;
                }
                return reward.base * reward.multiplier;
            }

            fn main() {
                let reward = Reward {
                    enabled: true,
                    base: 12,
                    multiplier: 3,
                };
                return score_reward(reward) + 4;
            }
            "#,
            "main",
        ))
        .expect("valid playground response");

        assert_eq!(response["ok"], true, "{response:#}");
        assert_eq!(response["value"], 40);
    }
}
