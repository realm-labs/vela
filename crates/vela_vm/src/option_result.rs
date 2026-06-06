use crate::heap::HeapValue;
use crate::owned_value::OwnedValue;
use crate::script_object::ScriptFields;
use crate::{
    ExecutionBudget, HeapExecution, Value, Vm, VmError, VmErrorKind, VmResult, allocate_heap_value,
};
use vela_common::standard_ids::{
    OPTION_FLATTEN_FUNCTION_ID, OPTION_IS_NONE_FUNCTION_ID, OPTION_IS_SOME_FUNCTION_ID,
    OPTION_NONE_FUNCTION_ID, OPTION_OK_OR_FUNCTION_ID, OPTION_SOME_FUNCTION_ID,
    OPTION_UNWRAP_OR_FUNCTION_ID, RESULT_ERR_FUNCTION_ID, RESULT_FLATTEN_FUNCTION_ID,
    RESULT_IS_ERR_FUNCTION_ID, RESULT_IS_OK_FUNCTION_ID, RESULT_OK_FUNCTION_ID,
    RESULT_TO_ERROR_OPTION_FUNCTION_ID, RESULT_TO_OPTION_FUNCTION_ID, RESULT_UNWRAP_OR_FUNCTION_ID,
};

pub(crate) fn register(vm: &mut Vm) {
    vm.register_native_with_id(OPTION_SOME_FUNCTION_ID, "option::some", option_some);
    vm.register_native_with_id(OPTION_NONE_FUNCTION_ID, "option::none", option_none);
    vm.register_native_with_id(
        OPTION_IS_SOME_FUNCTION_ID,
        "option::is_some",
        option_is_some,
    );
    vm.register_native_with_id(
        OPTION_IS_NONE_FUNCTION_ID,
        "option::is_none",
        option_is_none,
    );
    vm.register_native_with_id(
        OPTION_UNWRAP_OR_FUNCTION_ID,
        "option::unwrap_or",
        option_unwrap_or,
    );
    vm.register_native_with_id(OPTION_OK_OR_FUNCTION_ID, "option::ok_or", option_ok_or);
    vm.register_native_with_id(
        OPTION_FLATTEN_FUNCTION_ID,
        "option::flatten",
        option_flatten,
    );
    vm.register_native_with_id(RESULT_OK_FUNCTION_ID, "result::ok", result_ok);
    vm.register_native_with_id(RESULT_ERR_FUNCTION_ID, "result::err", result_err);
    vm.register_native_with_id(RESULT_IS_OK_FUNCTION_ID, "result::is_ok", result_is_ok);
    vm.register_native_with_id(RESULT_IS_ERR_FUNCTION_ID, "result::is_err", result_is_err);
    vm.register_native_with_id(
        RESULT_UNWRAP_OR_FUNCTION_ID,
        "result::unwrap_or",
        result_unwrap_or,
    );
    vm.register_native_with_id(
        RESULT_TO_OPTION_FUNCTION_ID,
        "result::to_option",
        result_to_option,
    );
    vm.register_native_with_id(
        RESULT_TO_ERROR_OPTION_FUNCTION_ID,
        "result::to_error_option",
        result_to_error_option,
    );
    vm.register_native_with_id(
        RESULT_FLATTEN_FUNCTION_ID,
        "result::flatten",
        result_flatten,
    );
}

pub(crate) fn option_value(
    payload: Option<Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    match payload {
        Some(payload) => {
            enum_heap_payload_value("Option", "Some", "Option::Some", payload, heap, budget)
        }
        None => option_none_value(heap, budget),
    }
}

pub(crate) fn option_none_value(
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    enum_heap_empty_value("Option", "None", "Option::None", heap, budget)
}

pub(crate) fn result_value(
    variant: &str,
    payload: Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    let owner = match variant {
        "Ok" => "Result::Ok",
        "Err" => "Result::Err",
        _ => return type_error("method result"),
    };
    enum_heap_payload_value("Result", variant, owner, payload, heap, budget)
}

fn enum_heap_empty_value(
    enum_name: &str,
    variant: &str,
    owner: &str,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    enum_heap_value(enum_name, variant, ScriptFields::empty(owner), heap, budget)
}

fn enum_heap_payload_value(
    enum_name: &str,
    variant: &str,
    owner: &str,
    payload: Value,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    enum_heap_value(
        enum_name,
        variant,
        ScriptFields::single(owner, "0", payload),
        heap,
        budget,
    )
}

fn enum_heap_value(
    enum_name: &str,
    variant: &str,
    fields: ScriptFields<Value>,
    heap: &mut HeapExecution<'_>,
    budget: Option<&mut ExecutionBudget>,
) -> VmResult<Value> {
    allocate_heap_value(
        HeapValue::Enum {
            enum_name: enum_name.to_owned(),
            variant: variant.to_owned(),
            fields,
        },
        heap,
        budget,
    )
}

fn owned_option_value(payload: Option<OwnedValue>) -> OwnedValue {
    match payload {
        Some(value) => owned_enum_payload_value("Option", "Some", "Option::Some", value),
        None => owned_enum_empty_value("Option", "None", "Option::None"),
    }
}

fn option_some(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::some", args, 1)?;
    Ok(owned_option_value(Some(args[0].clone())))
}

fn option_none(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::none", args, 0)?;
    Ok(owned_option_value(None))
}

fn option_is_some(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::is_some", args, 1)?;
    option_variant(&args[0], "option::is_some").map(|variant| OwnedValue::Bool(variant == "Some"))
}

fn option_is_none(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::is_none", args, 1)?;
    option_variant(&args[0], "option::is_none").map(|variant| OwnedValue::Bool(variant == "None"))
}

fn option_unwrap_or(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::unwrap_or", args, 2)?;
    match option_variant(&args[0], "option::unwrap_or")? {
        "Some" => enum_payload(&args[0], "option::unwrap_or"),
        "None" => Ok(args[1].clone()),
        _ => type_error("option::unwrap_or"),
    }
}

fn option_ok_or(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::ok_or", args, 2)?;
    match option_variant(&args[0], "option::ok_or")? {
        "Some" => {
            enum_payload(&args[0], "option::ok_or").map(|payload| owned_result_value("Ok", payload))
        }
        "None" => Ok(owned_result_value("Err", args[1].clone())),
        _ => type_error("option::ok_or"),
    }
}

fn option_flatten(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("option::flatten", args, 1)?;
    match option_variant(&args[0], "option::flatten")? {
        "Some" => {
            let payload = enum_payload(&args[0], "option::flatten")?;
            option_variant(&payload, "option::flatten")?;
            Ok(payload)
        }
        "None" => Ok(owned_option_value(None)),
        _ => type_error("option::flatten"),
    }
}

fn result_ok(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::ok", args, 1)?;
    Ok(owned_result_value("Ok", args[0].clone()))
}

fn result_err(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::err", args, 1)?;
    Ok(owned_result_value("Err", args[0].clone()))
}

fn result_is_ok(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::is_ok", args, 1)?;
    result_variant(&args[0], "result::is_ok").map(|variant| OwnedValue::Bool(variant == "Ok"))
}

fn result_is_err(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::is_err", args, 1)?;
    result_variant(&args[0], "result::is_err").map(|variant| OwnedValue::Bool(variant == "Err"))
}

fn result_unwrap_or(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::unwrap_or", args, 2)?;
    match result_variant(&args[0], "result::unwrap_or")? {
        "Ok" => enum_payload(&args[0], "result::unwrap_or"),
        "Err" => Ok(args[1].clone()),
        _ => type_error("result::unwrap_or"),
    }
}

fn result_to_option(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::to_option", args, 1)?;
    match result_variant(&args[0], "result::to_option")? {
        "Ok" => enum_payload(&args[0], "result::to_option")
            .map(Some)
            .map(owned_option_value),
        "Err" => Ok(owned_option_value(None)),
        _ => type_error("result::to_option"),
    }
}

fn result_to_error_option(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::to_error_option", args, 1)?;
    match result_variant(&args[0], "result::to_error_option")? {
        "Ok" => Ok(owned_option_value(None)),
        "Err" => enum_payload(&args[0], "result::to_error_option")
            .map(Some)
            .map(owned_option_value),
        _ => type_error("result::to_error_option"),
    }
}

fn result_flatten(args: &[OwnedValue]) -> VmResult<OwnedValue> {
    expect_arity("result::flatten", args, 1)?;
    match result_variant(&args[0], "result::flatten")? {
        "Ok" => {
            let payload = enum_payload(&args[0], "result::flatten")?;
            result_variant(&payload, "result::flatten")?;
            Ok(payload)
        }
        "Err" => enum_payload(&args[0], "result::flatten")
            .map(|payload| owned_result_value("Err", payload)),
        _ => type_error("result::flatten"),
    }
}

fn owned_result_value(variant: &str, payload: OwnedValue) -> OwnedValue {
    let owner = match variant {
        "Ok" => "Result::Ok",
        "Err" => "Result::Err",
        _ => "Result",
    };
    owned_enum_payload_value("Result", variant, owner, payload)
}

fn owned_enum_empty_value(enum_name: &str, variant: &str, owner: &str) -> OwnedValue {
    owned_enum_value(enum_name, variant, ScriptFields::empty(owner))
}

fn owned_enum_payload_value(
    enum_name: &str,
    variant: &str,
    owner: &str,
    payload: OwnedValue,
) -> OwnedValue {
    owned_enum_value(
        enum_name,
        variant,
        ScriptFields::single(owner, "0", payload),
    )
}

fn owned_enum_value(
    enum_name: &str,
    variant: &str,
    fields: ScriptFields<OwnedValue>,
) -> OwnedValue {
    OwnedValue::Enum {
        enum_name: enum_name.to_owned(),
        variant: variant.to_owned(),
        fields,
    }
}

fn option_variant<'a>(value: &'a OwnedValue, operation: &'static str) -> VmResult<&'a str> {
    let (enum_name, variant) =
        enum_tag(value).ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if enum_name == "Option" || enum_name.rsplit("::").next() == Some("Option") {
        return Ok(variant);
    }
    type_error(operation)
}

fn result_variant<'a>(value: &'a OwnedValue, operation: &'static str) -> VmResult<&'a str> {
    let (enum_name, variant) =
        enum_tag(value).ok_or_else(|| VmError::new(VmErrorKind::TypeMismatch { operation }))?;
    if enum_name == "Result" || enum_name.rsplit("::").next() == Some("Result") {
        return Ok(variant);
    }
    type_error(operation)
}

fn enum_tag(value: &OwnedValue) -> Option<(&str, &str)> {
    match value {
        OwnedValue::Enum {
            enum_name, variant, ..
        } => Some((enum_name.as_str(), variant.as_str())),
        _ => None,
    }
}

fn enum_payload(value: &OwnedValue, operation: &'static str) -> VmResult<OwnedValue> {
    let OwnedValue::Enum { fields, .. } = value else {
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
