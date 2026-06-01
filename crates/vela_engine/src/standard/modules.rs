use vela_reflect::ModuleDesc;

pub(crate) fn standard_module_descs() -> [ModuleDesc; 4] {
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
    ]
}

fn stdlib_module(name: &'static str, docs: &'static str, namespace: &'static str) -> ModuleDesc {
    ModuleDesc::new(name).docs(docs).attr("stdlib", namespace)
}
