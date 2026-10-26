use super::Workload;

pub(crate) const EXTENDED_WORKLOADS: &[Workload] = &[
    Workload {
        name: "recursive_countdown",
        vela: include_str!("../../fixtures/external_compare/recursive_countdown.vela"),
        lua: include_str!("../../fixtures/external_compare/recursive_countdown.lua"),
        rhai: include_str!("../../fixtures/external_compare/recursive_countdown.rhai"),
        node: include_str!("../../fixtures/external_compare/recursive_countdown.js"),
        python: include_str!("../../fixtures/external_compare/recursive_countdown.py"),
    },
    Workload {
        name: "nested_collection_allocation",
        vela: include_str!("../../fixtures/external_compare/nested_collection_allocation.vela"),
        lua: include_str!("../../fixtures/external_compare/nested_collection_allocation.lua"),
        rhai: include_str!("../../fixtures/external_compare/nested_collection_allocation.rhai"),
        node: include_str!("../../fixtures/external_compare/nested_collection_allocation.js"),
        python: include_str!("../../fixtures/external_compare/nested_collection_allocation.py"),
    },
    Workload {
        name: "object_field_methods",
        vela: include_str!("../../fixtures/external_compare/object_field_methods.vela"),
        lua: include_str!("../../fixtures/external_compare/object_field_methods.lua"),
        rhai: include_str!("../../fixtures/external_compare/object_field_methods.rhai"),
        node: include_str!("../../fixtures/external_compare/object_field_methods.js"),
        python: include_str!("../../fixtures/external_compare/object_field_methods.py"),
    },
    Workload {
        name: "string_build_split_join",
        vela: include_str!("../../fixtures/external_compare/string_build_split_join.vela"),
        lua: include_str!("../../fixtures/external_compare/string_build_split_join.lua"),
        rhai: include_str!("../../fixtures/external_compare/string_build_split_join.rhai"),
        node: include_str!("../../fixtures/external_compare/string_build_split_join.js"),
        python: include_str!("../../fixtures/external_compare/string_build_split_join.py"),
    },
    Workload {
        name: "float_math_loop",
        vela: include_str!("../../fixtures/external_compare/float_math_loop.vela"),
        lua: include_str!("../../fixtures/external_compare/float_math_loop.lua"),
        rhai: include_str!("../../fixtures/external_compare/float_math_loop.rhai"),
        node: include_str!("../../fixtures/external_compare/float_math_loop.js"),
        python: include_str!("../../fixtures/external_compare/float_math_loop.py"),
    },
    Workload {
        name: "array_transform_sort",
        vela: include_str!("../../fixtures/external_compare/array_transform_sort.vela"),
        lua: include_str!("../../fixtures/external_compare/array_transform_sort.lua"),
        rhai: include_str!("../../fixtures/external_compare/array_transform_sort.rhai"),
        node: include_str!("../../fixtures/external_compare/array_transform_sort.js"),
        python: include_str!("../../fixtures/external_compare/array_transform_sort.py"),
    },
];
