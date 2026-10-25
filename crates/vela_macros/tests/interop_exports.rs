use vela_engine::args::{FromScriptArg, IntoScriptArg, ToScriptValueRef};
use vela_engine::context::NativeCallContext;
use vela_engine::engine::Engine;
use vela_engine::interop::{BoundaryMode, CallableKind};
use vela_engine::native::EffectSet;
use vela_engine::registration::VelaBindings;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_macros::{Value, export, export_module, external_value_enum};
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, Eq, PartialEq, Value)]
#[vela(path = "data::Record")]
pub struct Record {
    count: i64,
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalQuality {
    Normal,
    Rare,
}

external_value_enum! {
    path = "data::Quality",
    ty = ExternalQuality,
    variants = [Normal, Rare],
}

#[export_module(path = "rules")]
mod rules_exports {
    pub fn clamp(amount: i64) -> i64 {
        amount.max(0)
    }

    #[vela(effects(random))]
    pub fn random_floor() -> i64 {
        1
    }

    pub(super) fn private_helper() -> i64 {
        2
    }
}

#[export(path = "data::normalize")]
pub fn normalize(amount: i64) -> i64 {
    amount.max(0)
}

#[export(path = "data::inspect")]
pub fn inspect(record: &Record, suffix: &str) -> String {
    format!("{}:{}{}", record.count, record.name, suffix)
}

#[export(path = "data::owned")]
pub fn owned(mut record: Record) -> Record {
    record.count += 1;
    record
}

#[export(path = "data::quality")]
pub fn quality() -> ExternalQuality {
    ExternalQuality::Rare
}

#[export(path = "data::quality_name")]
pub fn quality_name(value: &ExternalQuality) -> String {
    format!("{value:?}")
}

#[export(path = "data::fail")]
pub fn fail() -> VmResult<()> {
    Err(VmError::new(VmErrorKind::TypeMismatch {
        operation: "authored Rust failure",
    }))
}

#[export(path = "data::panic")]
pub fn panic_export() {
    panic!("authored Rust panic payload must not cross the boundary");
}

#[export(path = "data::panic_shared")]
pub fn panic_shared(_record: &Record) {
    panic!("shared decoded argument must not change panic conversion");
}

#[export(path = "data::context", effects(random))]
pub fn context(_ctx: &mut NativeCallContext<'_>, value: &Record, suffix: &str) -> String {
    inspect(value, suffix)
}

fn runtime(source: &str) -> Runtime {
    let engine = Engine::builder()
        .install_generated_type::<Record>()
        .install_generated_type::<ExternalQuality>()
        .install_registration(vela_function_normalize())
        .install_registration(vela_function_inspect())
        .install_registration(vela_function_owned())
        .install_registration(vela_function_quality())
        .install_registration(vela_function_quality_name())
        .install_registration(vela_function_fail())
        .install_registration(vela_function_panic_export())
        .install_registration(vela_function_panic_shared())
        .install_registration(vela_function_context())
        .build()
        .expect("value exports should seal");
    let program = engine
        .compile_source(source)
        .expect("value calls should compile");
    Runtime::new(engine, program).expect("runtime should initialize")
}

fn call(runtime: &mut Runtime, name: &str, args: CallArgs) -> VmResult<OwnedValue> {
    let result = runtime.call(name, args, CallOptions::unbounded())?;
    runtime.value_to_owned(&result)
}

#[test]
fn ordinary_exports_emit_normalized_callable_contracts() {
    let normalize = vela_callable_contract_normalize();
    assert_eq!(normalize.identity.kind, CallableKind::RustFunction);
    assert_eq!(normalize.effects, EffectSet::pure());
    assert_eq!(normalize.parameters[0].mode, BoundaryMode::Value);
    let inspect = vela_callable_contract_inspect();
    assert_eq!(inspect.effects, EffectSet::pure());
    assert!(
        inspect
            .parameters
            .iter()
            .all(|p| p.mode == BoundaryMode::ReadOnlyValueBorrow)
    );
    let context = vela_callable_contract_context();
    assert_eq!(context.effects, EffectSet::random());
    assert_eq!(context.parameters[0].mode, BoundaryMode::HiddenContext);
}

#[test]
fn export_module_groups_public_contracts_once() {
    let bundle = rules_exports::vela_module();
    let contracts = bundle.contracts();
    assert_eq!(contracts.len(), 2);
    assert_eq!(contracts[0].public_path, "rules::clamp");
    assert_eq!(contracts[1].public_path, "rules::random_floor");
    assert_eq!(contracts[1].effects, EffectSet::random());
    assert_eq!(rules_exports::clamp(-2), 0);
    assert_eq!(rules_exports::random_floor(), 1);
    assert_eq!(rules_exports::private_helper(), 2);
}

#[test]
fn module_registration_installs_through_application_bindings() {
    let mut bindings = VelaBindings::new();
    bindings.register_module(rules_exports::vela_module());
    let engine = Engine::builder()
        .register_bindings(bindings)
        .build()
        .expect("module installs");
    let program = engine
        .compile_source("fn main() { return rules::clamp(-7); }")
        .expect("module compiles");
    let mut runtime = Runtime::new(engine, program).expect("runtime initializes");
    assert_eq!(
        call(&mut runtime, "main", CallArgs::new()),
        Ok(OwnedValue::i64(0))
    );
}

#[test]
fn decoded_shared_values_and_strings_live_only_for_the_invocation() {
    let original = Record {
        count: 3,
        name: "token".to_owned(),
    };
    let mut runtime = runtime(
        r#"
fn inspect(value: data::Record) { return data::inspect(value, "!"); }
fn transform(value: data::Record) {
    value.count = 8;
    return data::owned(value);
}
"#,
    );
    assert_eq!(
        call(
            &mut runtime,
            "inspect",
            CallArgs::from_positional([original.to_script_value_ref()])
        ),
        Ok(OwnedValue::from("3:token!")),
    );
    let transformed = call(
        &mut runtime,
        "transform",
        CallArgs::from_positional([original.to_script_value_ref()]),
    )
    .expect("owned transform");
    assert_eq!(
        Record::from_script_arg(&transformed)
            .expect("decoded record")
            .count,
        9
    );
    assert_eq!(
        original,
        Record {
            count: 3,
            name: "token".to_owned()
        }
    );
}

#[test]
fn external_value_enum_keeps_owned_codecs_registration_and_match_identity() {
    let original = ExternalQuality::Rare;
    assert_eq!(
        ExternalQuality::from_script_arg(&original.to_script_value_ref()),
        Ok(original)
    );
    assert_eq!(
        ExternalQuality::from_script_arg(&original.into_script_arg()),
        Ok(original)
    );
    let mut runtime = runtime(
        r#"
fn main() {
    let value = data::quality();
    return match value {
        data::Quality::Rare => data::quality_name(value),
        _ => "wrong",
    };
}
"#,
    );
    assert_eq!(
        call(&mut runtime, "main", CallArgs::new()),
        Ok(OwnedValue::from("Rare"))
    );
}

#[test]
fn exported_value_errors_and_panics_are_converted_and_runtime_remains_usable() {
    let mut runtime = runtime(
        r#"
fn failure() { data::fail(); }
fn panic_owned() { data::panic(); }
fn panic_shared() { data::panic_shared(data::Record { count: 1, name: "x" }); }
fn normal() { return data::normalize(-4); }
"#,
    );
    let error = call(&mut runtime, "failure", CallArgs::new()).expect_err("authored error");
    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "authored Rust failure"
        }
    ));
    for (entry, callable) in [
        ("panic_owned", "data::panic"),
        ("panic_shared", "data::panic_shared"),
    ] {
        let error = call(&mut runtime, entry, CallArgs::new()).expect_err("panic converted");
        assert!(
            matches!(error.kind(), VmErrorKind::RustCallablePanicked { callable: name } if name == callable)
        );
        assert_eq!(
            call(&mut runtime, "normal", CallArgs::new()),
            Ok(OwnedValue::i64(0))
        );
    }
}
