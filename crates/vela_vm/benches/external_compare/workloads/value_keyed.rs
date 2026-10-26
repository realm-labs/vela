use super::Workload;

pub(crate) const VALUE_KEYED_WORKLOADS: &[Workload] = &[
    Workload {
        name: "map_string_index_lookup_update",
        vela: include_str!("../../fixtures/external_compare/map_string_index_lookup_update.vela"),
        lua: include_str!("../../fixtures/external_compare/map_string_index_lookup_update.lua"),
        rhai: include_str!("../../fixtures/external_compare/map_lookup_update.rhai"),
        node: include_str!("../../fixtures/external_compare/map_string_index_lookup_update.js"),
        python: include_str!("../../fixtures/external_compare/map_string_index_lookup_update.py"),
    },
    Workload {
        name: "map_string_key_lookup_update",
        vela: include_str!("../../fixtures/external_compare/map_string_key_lookup_update.vela"),
        lua: include_str!("../../fixtures/external_compare/map_string_key_lookup_update.lua"),
        rhai: include_str!("../../fixtures/external_compare/map_lookup_update.rhai"),
        node: include_str!("../../fixtures/external_compare/map_string_key_lookup_update.js"),
        python: include_str!("../../fixtures/external_compare/map_string_key_lookup_update.py"),
    },
    Workload {
        name: "map_i64_key_lookup_update",
        vela: include_str!("../../fixtures/external_compare/map_i64_key_lookup_update.vela"),
        lua: include_str!("../../fixtures/external_compare/map_i64_key_lookup_update.lua"),
        rhai: include_str!("../../fixtures/external_compare/map_i64_key_lookup_update.rhai"),
        node: include_str!("../../fixtures/external_compare/map_i64_key_lookup_update.js"),
        python: include_str!("../../fixtures/external_compare/map_i64_key_lookup_update.py"),
    },
    Workload {
        name: "map_record_identity_lookup_update",
        vela: include_str!(
            "../../fixtures/external_compare/map_record_identity_lookup_update.vela"
        ),
        lua: include_str!("../../fixtures/external_compare/map_record_identity_lookup_update.lua"),
        rhai: include_str!(
            "../../fixtures/external_compare/map_record_identity_lookup_update.rhai"
        ),
        node: include_str!("../../fixtures/external_compare/map_record_identity_lookup_update.js"),
        python: include_str!(
            "../../fixtures/external_compare/map_record_identity_lookup_update.py"
        ),
    },
    Workload {
        name: "set_i64_lookup_mutation",
        vela: include_str!("../../fixtures/external_compare/set_i64_lookup_mutation.vela"),
        lua: include_str!("../../fixtures/external_compare/set_i64_lookup_mutation.lua"),
        rhai: include_str!("../../fixtures/external_compare/set_i64_lookup_mutation.rhai"),
        node: include_str!("../../fixtures/external_compare/set_i64_lookup_mutation.js"),
        python: include_str!("../../fixtures/external_compare/set_i64_lookup_mutation.py"),
    },
    Workload {
        name: "set_i64_large_lookup_mutation",
        vela: include_str!("../../fixtures/external_compare/set_i64_large_lookup_mutation.vela"),
        lua: include_str!("../../fixtures/external_compare/set_i64_large_lookup_mutation.lua"),
        rhai: include_str!("../../fixtures/external_compare/set_i64_large_lookup_mutation.rhai"),
        node: include_str!("../../fixtures/external_compare/set_i64_large_lookup_mutation.js"),
        python: include_str!("../../fixtures/external_compare/set_i64_large_lookup_mutation.py"),
    },
    Workload {
        name: "set_string_lookup_mutation",
        vela: include_str!("../../fixtures/external_compare/set_lookup_mutation.vela"),
        lua: include_str!("../../fixtures/external_compare/set_lookup_mutation.lua"),
        rhai: include_str!("../../fixtures/external_compare/set_lookup_mutation.rhai"),
        node: include_str!("../../fixtures/external_compare/set_lookup_mutation.js"),
        python: include_str!("../../fixtures/external_compare/set_lookup_mutation.py"),
    },
    Workload {
        name: "set_record_identity_lookup_mutation",
        vela: include_str!(
            "../../fixtures/external_compare/set_record_identity_lookup_mutation.vela"
        ),
        lua: include_str!(
            "../../fixtures/external_compare/set_record_identity_lookup_mutation.lua"
        ),
        rhai: include_str!(
            "../../fixtures/external_compare/set_record_identity_lookup_mutation.rhai"
        ),
        node: include_str!(
            "../../fixtures/external_compare/set_record_identity_lookup_mutation.js"
        ),
        python: include_str!(
            "../../fixtures/external_compare/set_record_identity_lookup_mutation.py"
        ),
    },
];
