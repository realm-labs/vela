use vela_reflect::modules::ModuleDesc;

pub(crate) fn standard_module_descs() -> [ModuleDesc; 11] {
    [
        stdlib_module(
            "math",
            "Deterministic math standard-library helpers.",
            "math",
        ),
        stdlib_module(
            "option",
            "Option standard-library propagation helpers.",
            "option",
        ),
        stdlib_module(
            "result",
            "Result standard-library propagation helpers.",
            "result",
        ),
        stdlib_module("set", "Set standard-library construction helpers.", "set"),
        stdlib_module(
            "bytes",
            "Bytes standard-library conversion helpers.",
            "bytes",
        ),
        stdlib_module("i64", "i64 primitive conversion helpers.", "i64"),
        stdlib_module("u64", "u64 primitive conversion helpers.", "u64"),
        stdlib_module("f64", "f64 primitive conversion helpers.", "f64"),
        stdlib_module("i8", "i8 primitive conversion helpers.", "i8"),
        stdlib_module("u8", "u8 primitive conversion helpers.", "u8"),
        stdlib_module("f32", "f32 primitive conversion helpers.", "f32"),
    ]
}

fn stdlib_module(name: &'static str, docs: &'static str, namespace: &'static str) -> ModuleDesc {
    ModuleDesc::new(name).docs(docs).attr("stdlib", namespace)
}
