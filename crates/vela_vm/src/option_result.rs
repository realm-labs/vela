use crate::script_object::ScriptFields;
use crate::{Value, Vm, VmError, VmErrorKind, VmResult, expect_arity};

pub(crate) fn register(vm: &mut Vm) {
    vm.register_native("option.some", option_some);
    vm.register_native("option.none", option_none);
    vm.register_native("option.is_some", option_is_some);
    vm.register_native("option.is_none", option_is_none);
    vm.register_native("option.unwrap_or", option_unwrap_or);
    vm.register_native("option.ok_or", option_ok_or);
    vm.register_native("result.ok", result_ok);
    vm.register_native("result.err", result_err);
    vm.register_native("result.is_ok", result_is_ok);
    vm.register_native("result.is_err", result_is_err);
    vm.register_native("result.unwrap_or", result_unwrap_or);
    vm.register_native("result.to_option", result_to_option);
}

pub(crate) fn option_value(payload: Option<Value>) -> Value {
    let (variant, fields) = match payload {
        Some(value) => ("Some", vec![("0".to_owned(), value)]),
        None => ("None", Vec::new()),
    };
    enum_value("Option", variant, fields)
}

fn option_some(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.some", args, 1)?;
    Ok(option_value(Some(args[0].clone())))
}

fn option_none(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.none", args, 0)?;
    Ok(option_value(None))
}

fn option_is_some(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.is_some", args, 1)?;
    option_variant(&args[0], "option.is_some").map(|variant| Value::Bool(variant == "Some"))
}

fn option_is_none(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.is_none", args, 1)?;
    option_variant(&args[0], "option.is_none").map(|variant| Value::Bool(variant == "None"))
}

fn option_unwrap_or(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.unwrap_or", args, 2)?;
    match option_variant(&args[0], "option.unwrap_or")? {
        "Some" => enum_payload(&args[0], "option.unwrap_or"),
        "None" => Ok(args[1].clone()),
        _ => type_error("option.unwrap_or"),
    }
}

fn option_ok_or(args: &[Value]) -> VmResult<Value> {
    expect_arity("option.ok_or", args, 2)?;
    match option_variant(&args[0], "option.ok_or")? {
        "Some" => enum_payload(&args[0], "option.ok_or").map(|payload| result_value("Ok", payload)),
        "None" => Ok(result_value("Err", args[1].clone())),
        _ => type_error("option.ok_or"),
    }
}

fn result_ok(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.ok", args, 1)?;
    Ok(result_value("Ok", args[0].clone()))
}

fn result_err(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.err", args, 1)?;
    Ok(result_value("Err", args[0].clone()))
}

fn result_is_ok(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.is_ok", args, 1)?;
    result_variant(&args[0], "result.is_ok").map(|variant| Value::Bool(variant == "Ok"))
}

fn result_is_err(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.is_err", args, 1)?;
    result_variant(&args[0], "result.is_err").map(|variant| Value::Bool(variant == "Err"))
}

fn result_unwrap_or(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.unwrap_or", args, 2)?;
    match result_variant(&args[0], "result.unwrap_or")? {
        "Ok" => enum_payload(&args[0], "result.unwrap_or"),
        "Err" => Ok(args[1].clone()),
        _ => type_error("result.unwrap_or"),
    }
}

fn result_to_option(args: &[Value]) -> VmResult<Value> {
    expect_arity("result.to_option", args, 1)?;
    match result_variant(&args[0], "result.to_option")? {
        "Ok" => enum_payload(&args[0], "result.to_option")
            .map(Some)
            .map(option_value),
        "Err" => Ok(option_value(None)),
        _ => type_error("result.to_option"),
    }
}

pub(crate) fn result_value(variant: &str, payload: Value) -> Value {
    enum_value("Result", variant, vec![("0".to_owned(), payload)])
}

fn enum_value(enum_name: &str, variant: &str, fields: Vec<(String, Value)>) -> Value {
    Value::Enum {
        enum_name: enum_name.to_owned(),
        variant: variant.to_owned(),
        fields: ScriptFields::from_pairs(&format!("{enum_name}.{variant}"), fields),
    }
}

fn option_variant<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    let (enum_name, variant) =
        enum_tag(value).ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if enum_name == "Option" || enum_name.rsplit('.').next() == Some("Option") {
        return Ok(variant);
    }
    type_error(operation)
}

fn result_variant<'a>(value: &'a Value, operation: &'static str) -> VmResult<&'a str> {
    let (enum_name, variant) =
        enum_tag(value).ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if enum_name == "Result" || enum_name.rsplit('.').next() == Some("Result") {
        return Ok(variant);
    }
    type_error(operation)
}

fn enum_tag(value: &Value) -> Option<(&str, &str)> {
    match value {
        Value::Enum {
            enum_name, variant, ..
        } => Some((enum_name.as_str(), variant.as_str())),
        _ => None,
    }
}

fn enum_payload(value: &Value, operation: &'static str) -> VmResult<Value> {
    let Value::Enum { fields, .. } = value else {
        return type_error(operation);
    };
    fields
        .get("0")
        .cloned()
        .ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}
