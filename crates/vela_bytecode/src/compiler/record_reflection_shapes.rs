use super::record_shapes::{RecordShape, ValueShape};

pub(super) fn native_call_shape(
    function: &str,
    first_arg: Option<ValueShape>,
) -> Option<ValueShape> {
    match function {
        "attrs" => Some(ValueShape::Map {
            key: Box::new(string_shape()),
            value: Box::new(ValueShape::Unknown),
        }),
        "access" => reflect_access_shape(first_arg?),
        "effects" => Some(effects_record_shape()),
        "field" => Some(field_record_shape()),
        "fields" => Some(ValueShape::Array(Box::new(field_record_shape()))),
        "function" => Some(function_record_shape()),
        "functions" => Some(ValueShape::Array(Box::new(function_record_shape()))),
        "module" => Some(module_record_shape()),
        "modules" => Some(ValueShape::Array(Box::new(module_record_shape()))),
        "method" => Some(method_record_shape()),
        "methods" => Some(ValueShape::Array(Box::new(method_record_shape()))),
        "params" => Some(ValueShape::Array(Box::new(param_record_shape()))),
        "source_span" => Some(source_span_record_shape()),
        "trait_info" => Some(trait_record_shape()),
        "traits" => Some(ValueShape::Array(Box::new(trait_record_shape()))),
        "type_info" | "type_of" => Some(type_record_shape()),
        "types" => Some(ValueShape::Array(Box::new(type_record_shape()))),
        "variant_info" => Some(variant_record_shape()),
        "variants" => Some(ValueShape::Array(Box::new(variant_record_shape()))),
        "exports" | "permissions" | "required_permissions" => {
            Some(ValueShape::Array(Box::new(string_shape())))
        }
        _ => None,
    }
}

fn reflect_access_shape(target: ValueShape) -> Option<ValueShape> {
    if let Some(access) = target
        .as_record()
        .and_then(|record| record.field_value_shape("access"))
        .cloned()
    {
        return Some(access);
    }
    let record = target.as_record()?;
    let has_required_permissions = record.field_slot("required_permissions").is_some();
    let has_access_flag = [
        "reflect_callable",
        "reflect_readable",
        "reflect_visible",
        "reflect_writable",
    ]
    .iter()
    .any(|field| record.field_slot(field).is_some());
    (has_required_permissions && has_access_flag).then_some(target)
}

fn effects_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("calls_reflection".to_owned(), bool_shape()),
        ("emits_events".to_owned(), bool_shape()),
        ("reads_host".to_owned(), bool_shape()),
        ("reads_reflection".to_owned(), bool_shape()),
        ("reads_time".to_owned(), bool_shape()),
        ("reads_io".to_owned(), bool_shape()),
        ("uses_random".to_owned(), bool_shape()),
        ("writes_host".to_owned(), bool_shape()),
        ("writes_reflection".to_owned(), bool_shape()),
        ("writes_io".to_owned(), bool_shape()),
    ]))
}

fn module_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("attrs".to_owned(), attrs_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        (
            "exports".to_owned(),
            ValueShape::Array(Box::new(string_shape())),
        ),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        ("source_span".to_owned(), source_span_record_shape()),
    ]))
}

fn field_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("access".to_owned(), field_access_record_shape()),
        ("attrs".to_owned(), attrs_shape()),
        ("defaulted".to_owned(), bool_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        ("id".to_owned(), i64_shape()),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        ("owner".to_owned(), string_shape()),
        ("source_span".to_owned(), ValueShape::Unknown),
        ("type".to_owned(), ValueShape::Unknown),
        ("writable".to_owned(), bool_shape()),
    ]))
}

fn field_access_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("readable".to_owned(), bool_shape()),
        ("reflect_readable".to_owned(), bool_shape()),
        ("reflect_writable".to_owned(), bool_shape()),
        (
            "required_permissions".to_owned(),
            ValueShape::Array(Box::new(string_shape())),
        ),
        ("writable".to_owned(), bool_shape()),
    ]))
}

fn function_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("access".to_owned(), function_access_record_shape()),
        ("attrs".to_owned(), attrs_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        ("effects".to_owned(), effects_record_shape()),
        ("id".to_owned(), i64_shape()),
        ("module".to_owned(), ValueShape::Unknown),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        (
            "params".to_owned(),
            ValueShape::Array(Box::new(param_record_shape())),
        ),
        ("public".to_owned(), bool_shape()),
        ("return".to_owned(), ValueShape::Unknown),
        ("returns".to_owned(), ValueShape::Unknown),
        ("source_span".to_owned(), ValueShape::Unknown),
    ]))
}

fn function_access_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("public".to_owned(), bool_shape()),
        ("reflect_callable".to_owned(), bool_shape()),
        ("reflect_visible".to_owned(), bool_shape()),
        (
            "required_permissions".to_owned(),
            ValueShape::Array(Box::new(string_shape())),
        ),
    ]))
}

fn method_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("access".to_owned(), method_access_record_shape()),
        ("attrs".to_owned(), attrs_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        ("effects".to_owned(), effects_record_shape()),
        ("id".to_owned(), i64_shape()),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        ("owner".to_owned(), string_shape()),
        (
            "params".to_owned(),
            ValueShape::Array(Box::new(param_record_shape())),
        ),
        ("return".to_owned(), ValueShape::Unknown),
        ("returns".to_owned(), ValueShape::Unknown),
        ("source_span".to_owned(), ValueShape::Unknown),
    ]))
}

fn method_access_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("public".to_owned(), bool_shape()),
        ("reflect_callable".to_owned(), bool_shape()),
        (
            "required_permissions".to_owned(),
            ValueShape::Array(Box::new(string_shape())),
        ),
    ]))
}

fn param_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("defaulted".to_owned(), bool_shape()),
        ("name".to_owned(), string_shape()),
        ("type".to_owned(), ValueShape::Unknown),
    ]))
}

fn source_span_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("end".to_owned(), i64_shape()),
        ("source".to_owned(), i64_shape()),
        ("start".to_owned(), i64_shape()),
    ]))
}

fn trait_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("attrs".to_owned(), attrs_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        ("id".to_owned(), i64_shape()),
        (
            "methods".to_owned(),
            ValueShape::Array(Box::new(method_record_shape())),
        ),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        ("source_span".to_owned(), source_span_record_shape()),
    ]))
}

fn type_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("attrs".to_owned(), attrs_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        ("field_count".to_owned(), i64_shape()),
        ("id".to_owned(), i64_shape()),
        ("kind".to_owned(), string_shape()),
        ("method_count".to_owned(), i64_shape()),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        ("schema_hash".to_owned(), ValueShape::Unknown),
        ("source_span".to_owned(), source_span_record_shape()),
        ("trait_count".to_owned(), i64_shape()),
        ("variant_count".to_owned(), i64_shape()),
    ]))
}

fn variant_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("attrs".to_owned(), attrs_shape()),
        ("docs".to_owned(), ValueShape::Unknown),
        (
            "fields".to_owned(),
            ValueShape::Array(Box::new(field_record_shape())),
        ),
        ("id".to_owned(), i64_shape()),
        ("name".to_owned(), string_shape()),
        ("origin".to_owned(), string_shape()),
        ("owner".to_owned(), string_shape()),
        ("source_span".to_owned(), ValueShape::Unknown),
    ]))
}

fn attrs_shape() -> ValueShape {
    ValueShape::Map {
        key: Box::new(string_shape()),
        value: Box::new(ValueShape::Unknown),
    }
}

fn bool_shape() -> ValueShape {
    ValueShape::Scalar("bool".to_owned())
}

fn i64_shape() -> ValueShape {
    ValueShape::Scalar("i64".to_owned())
}

fn string_shape() -> ValueShape {
    ValueShape::Scalar("String".to_owned())
}
