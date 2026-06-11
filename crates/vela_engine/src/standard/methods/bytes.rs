use vela_reflect::registry::MethodDesc;

use super::{MethodSpec, ParamSpec, descs};

pub(crate) fn bytes_method_descs() -> Vec<MethodDesc> {
    descs("Bytes", BYTES_METHODS, "bytes")
}

const BYTES_METHODS: &[MethodSpec] = &[
    MethodSpec::new("len", &[], "i64", "Returns the byte length."),
    MethodSpec::new(
        "is_empty",
        &[],
        "bool",
        "Returns true when the byte buffer has no bytes.",
    ),
    MethodSpec::new(
        "slice",
        &[ParamSpec::new("start", "i64"), ParamSpec::new("end", "i64")],
        "bytes",
        "Returns the byte range as a new byte buffer.",
    ),
    MethodSpec::new(
        "get",
        &[ParamSpec::new("index", "i64")],
        "u8",
        "Returns the byte at the index.",
    ),
    MethodSpec::new(
        "read_u32_le",
        &[ParamSpec::new("index", "i64")],
        "u32",
        "Reads four bytes at the index as little-endian u32.",
    ),
    MethodSpec::new(
        "read_u32_be",
        &[ParamSpec::new("index", "i64")],
        "u32",
        "Reads four bytes at the index as big-endian u32.",
    ),
    MethodSpec::new(
        "to_hex",
        &[],
        "string",
        "Returns the lowercase hexadecimal representation.",
    ),
];
