use std::collections::BTreeMap;

use vela_common::{FunctionId, SourceId, Span};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
use vela_host::value::HostValue;

use crate::access::{FunctionAccess, FunctionEffectSet};
use crate::error::ReflectErrorKind;
use crate::metadata::span_value;
use crate::permissions::ReflectPolicy;
use crate::registry::TypeRegistry;
use crate::value::ReflectValue;

use super::*;

#[test]
fn registers_script_module_functions_and_exports() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_dotted("game.reward"),
        r#"
pub fn grant(player: Player, amount: int = 1) -> bool {
    return true;
}

#[doc("Helper docs.")]
#[event("reward.helper")]
fn helper() {
    return null;
}
"#,
    ));
    let mut registry = TypeRegistry::new();

    registry.register_script_modules(&graph);

    let module = registry
        .module_by_name("game.reward")
        .expect("script module metadata");
    assert_eq!(module.exports.len(), 2);
    assert_eq!(module.origin, DeclOrigin::Script);
    assert_eq!(module.exports[0].name, "game.reward.grant");
    assert_eq!(module.exports[0].kind, ModuleExportKind::Function);
    assert_eq!(
        module.source_span.map(|span| span.source),
        Some(SourceId::new(1))
    );

    let grant = registry
        .function_by_name("game.reward.grant")
        .expect("grant function metadata");
    assert_eq!(grant.module.as_deref(), Some("game.reward"));
    assert!(grant.public);
    assert_eq!(grant.origin, DeclOrigin::Script);
    assert_eq!(grant.params[0].name, "player");
    assert_eq!(grant.params[0].type_hint.as_deref(), Some("Player"));
    assert_eq!(grant.params[1].name, "amount");
    assert_eq!(grant.params[1].type_hint.as_deref(), Some("int"));
    assert!(grant.params[1].has_default);
    assert_eq!(grant.return_type.as_deref(), Some("bool"));
    assert_eq!(
        grant.source_span.map(|span| span.source),
        Some(SourceId::new(1))
    );

    let helper = registry
        .function_by_name("game.reward.helper")
        .expect("helper function metadata");
    assert!(!helper.public);
    assert_eq!(helper.docs.as_deref(), Some("Helper docs."));
    assert_eq!(helper.attrs.get("event"), Some("reward.helper"));
}

#[test]
fn module_function_queries_return_records_and_candidates() {
    let mut registry = TypeRegistry::new();
    let function_id = FunctionId::new(7);
    let module_span = Span::new(SourceId::new(7), 10, 20);
    let function_span = Span::new(SourceId::new(7), 30, 50);
    registry.register_module(
        ModuleDesc::new("game.reward")
            .docs("Reward module.")
            .attr("domain", "gameplay")
            .source_span(module_span),
    );
    registry.register_function(
        FunctionDesc::new(function_id, "game.reward.grant")
            .module("game.reward")
            .param(
                FunctionParamDesc::new("amount")
                    .type_hint("int")
                    .defaulted(true),
            )
            .return_type("bool")
            .effects(FunctionEffectSet::host_write())
            .access(FunctionAccess::new().require_permission("reward.grant"))
            .origin(DeclOrigin::Script)
            .docs("Grant reward.")
            .attr("event", "reward")
            .source_span(function_span),
    );

    assert!(has_module(&registry, "game.reward"));
    assert!(!has_module(&registry, "game.missing"));
    assert!(has_function(&registry, "game.reward.grant"));
    assert!(!has_function(&registry, "game.reward.missing"));

    let module_value = module(&registry, "game.reward").expect("module");
    assert_eq!(
        crate::members::origin(&registry, &module_value).expect("module origin helper"),
        ReflectValue::Host(HostValue::String("host".to_owned()))
    );
    assert_eq!(
        exports_for_target(&registry, &module_value).expect("module record exports"),
        ReflectValue::Host(HostValue::Array(vec![HostValue::String(
            "game.reward.grant".into()
        )]))
    );
    assert_eq!(
        exports_for_target(
            &registry,
            &ReflectValue::Host(HostValue::String("game.reward".into())),
        )
        .expect("module string exports"),
        ReflectValue::Host(HostValue::Array(vec![HostValue::String(
            "game.reward.grant".into()
        )]))
    );
    let ReflectValue::Host(HostValue::Record {
        type_name,
        fields: module_metadata,
    }) = module_value
    else {
        panic!("module metadata should be a record");
    };
    assert_eq!(type_name, "ReflectModule");
    assert_eq!(
        module_metadata.get("name"),
        Some(&HostValue::String("game.reward".into()))
    );
    assert_eq!(
        module_metadata.get("origin"),
        Some(&HostValue::String("host".to_owned()))
    );
    assert_eq!(
        crate::members::docs(
            &registry,
            &ReflectValue::Host(HostValue::Record {
                type_name: type_name.clone(),
                fields: module_metadata.clone(),
            })
        )
        .expect("module docs helper"),
        ReflectValue::Host(HostValue::String("Reward module.".to_owned()))
    );
    assert_eq!(
        module_metadata.get("docs"),
        Some(&HostValue::String("Reward module.".to_owned()))
    );
    assert_eq!(
        module_metadata.get("attrs"),
        Some(&HostValue::Map(BTreeMap::from([(
            "domain".to_owned(),
            HostValue::String("gameplay".to_owned())
        )])))
    );
    assert_eq!(
        module_metadata.get("source_span"),
        Some(&span_value(Some(module_span)))
    );
    assert_eq!(
        exports(&registry, "game.reward").expect("exports"),
        ReflectValue::Host(HostValue::Array(vec![HostValue::String(
            "game.reward.grant".into()
        )]))
    );
    let ReflectValue::Host(HostValue::Array(modules)) = modules(&registry) else {
        panic!("module list should be an array");
    };
    assert_eq!(modules.len(), 1);
    let HostValue::Record {
        type_name,
        fields: module_list_item,
    } = &modules[0]
    else {
        panic!("module list item should be a record");
    };
    assert_eq!(type_name, "ReflectModule");
    assert_eq!(
        module_list_item.get("name"),
        Some(&HostValue::String("game.reward".into()))
    );
    assert_eq!(
        module_list_item.get("origin"),
        Some(&HostValue::String("host".to_owned()))
    );
    let ReflectValue::Host(HostValue::Array(functions)) = functions(&registry) else {
        panic!("function list should be an array");
    };
    assert_eq!(functions.len(), 1);
    let HostValue::Record {
        type_name,
        fields: function_list_item,
    } = &functions[0]
    else {
        panic!("function list item should be a record");
    };
    assert_eq!(type_name, "ReflectFunction");
    assert_eq!(
        function_list_item.get("name"),
        Some(&HostValue::String("game.reward.grant".into()))
    );
    assert_eq!(
        function_list_item.get("id"),
        Some(&HostValue::Int(
            i64::try_from(function_id.get()).unwrap_or(i64::MAX)
        ))
    );

    let function_value = function(&registry, "game.reward.grant").expect("function");
    assert_eq!(
        crate::members::origin(&registry, &function_value).expect("function origin helper"),
        ReflectValue::Host(HostValue::String("script".to_owned()))
    );
    let ReflectValue::Host(HostValue::Record {
        type_name,
        fields: function_metadata,
    }) = function_value
    else {
        panic!("function metadata should be a record");
    };
    assert_eq!(type_name, "ReflectFunction");
    assert_eq!(
        function_metadata.get("id"),
        Some(&HostValue::Int(
            i64::try_from(function_id.get()).unwrap_or(i64::MAX)
        ))
    );
    assert_eq!(
        function_metadata.get("return"),
        Some(&HostValue::String("bool".into()))
    );
    assert_eq!(
        function_metadata.get("origin"),
        Some(&HostValue::String("script".into()))
    );
    assert_eq!(
        function_metadata.get("source_span"),
        Some(&span_value(Some(function_span)))
    );
    assert_eq!(
        function_metadata.get("effects"),
        Some(&HostValue::Record {
            type_name: "ReflectEffectSet".to_owned(),
            fields: BTreeMap::from([
                ("reads_host".to_owned(), HostValue::Bool(true)),
                ("writes_host".to_owned(), HostValue::Bool(true)),
                ("emits_events".to_owned(), HostValue::Bool(false)),
            ]),
        })
    );
    assert_eq!(
        function_metadata.get("access"),
        Some(&HostValue::Record {
            type_name: "ReflectFunctionAccess".to_owned(),
            fields: BTreeMap::from([
                ("public".to_owned(), HostValue::Bool(true)),
                ("reflect_visible".to_owned(), HostValue::Bool(true)),
                ("reflect_callable".to_owned(), HostValue::Bool(false)),
                (
                    "required_permissions".to_owned(),
                    HostValue::Array(vec![HostValue::String("reward.grant".to_owned())])
                ),
            ]),
        })
    );
    assert_eq!(
        function_metadata.get("docs"),
        Some(&HostValue::String("Grant reward.".into()))
    );
    assert_eq!(
        function_metadata.get("attrs"),
        Some(&HostValue::Map(BTreeMap::from([(
            "event".to_owned(),
            HostValue::String("reward".to_owned())
        )])))
    );

    let error = module(&registry, "game.rewards").expect_err("unknown module");
    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownModule {
            module: "game.rewards".to_owned(),
            candidates: vec!["game.reward".to_owned()],
            related: vec![crate::candidates::ReflectCandidate::new(
                "game.reward",
                Some(module_span)
            )],
        }
    );

    let error = function(&registry, "game.reward.grnat").expect_err("unknown function");
    assert_eq!(
        error.kind,
        ReflectErrorKind::UnknownFunction {
            function: "game.reward.grnat".to_owned(),
            candidates: vec!["game.reward.grant".to_owned()],
            related: vec![crate::candidates::ReflectCandidate::new(
                "game.reward.grant",
                Some(function_span)
            )],
        }
    );
}

#[test]
fn function_policy_rejects_hidden_private_and_unapproved_functions() {
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game.hidden")
            .access(FunctionAccess::new().reflect_visible(false)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(2), "game.private")
            .access(FunctionAccess::new().public(false).reflect_visible(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game.admin")
            .access(FunctionAccess::new().require_permission("game.admin")),
    );
    let private_policy = ReflectPolicy::new(
        crate::permissions::ReflectPermissionSet::new()
            .with(crate::permissions::ReflectPermission::AccessPrivate),
    );

    let error = function_with_policy(&registry, "game.hidden", &ReflectPolicy::all())
        .expect_err("hidden function");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FunctionNotReflectVisible {
            function: "game.hidden".to_owned()
        }
    );

    let error = function_with_policy(&registry, "game.private", &ReflectPolicy::read_only())
        .expect_err("private function");
    assert_eq!(
        error.kind,
        ReflectErrorKind::PermissionDenied {
            permission: crate::permissions::ReflectPermission::AccessPrivate
        }
    );

    let error = function_with_policy(&registry, "game.admin", &private_policy)
        .expect_err("missing function permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FunctionPermissionDenied {
            function: "game.admin".to_owned(),
            permission: "game.admin".to_owned()
        }
    );
}

#[test]
fn function_policy_filters_unknown_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register_function(FunctionDesc::new(FunctionId::new(1), "game.reward.grant"));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(2), "game.reward.grant_hidden")
            .access(FunctionAccess::new().reflect_visible(false)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game.reward.grant_private")
            .access(FunctionAccess::new().public(false).reflect_visible(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(4), "game.reward.grant_admin")
            .access(FunctionAccess::new().require_permission("game.admin")),
    );

    let error = function_with_policy(
        &registry,
        "game.reward.grant_hiddden",
        &ReflectPolicy::read_only(),
    )
    .expect_err("unknown function");
    let ReflectErrorKind::UnknownFunction {
        candidates,
        related,
        ..
    } = error.kind
    else {
        panic!("expected unknown function");
    };

    assert_eq!(candidates, vec!["game.reward.grant".to_owned()]);
    assert_eq!(
        related,
        vec![crate::candidates::ReflectCandidate::new(
            "game.reward.grant",
            None
        )]
    );
}

#[test]
fn function_call_policy_filters_unknown_candidates() {
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game.reward.grant")
            .access(FunctionAccess::new().reflect_callable(true)),
    );
    registry.register_function(FunctionDesc::new(
        FunctionId::new(2),
        "game.reward.grant_visible",
    ));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game.reward.grant_write").access(
            FunctionAccess::new()
                .reflect_callable(true)
                .require_permission("game.write"),
        ),
    );

    let target = ReflectValue::Host(HostValue::Record {
        type_name: "ReflectFunction".to_owned(),
        fields: BTreeMap::from([(
            "name".to_owned(),
            HostValue::String("game.reward.grant_visibel".to_owned()),
        )]),
    });
    let error = callable_function_name_with_policy(&registry, &target, &ReflectPolicy::read_only())
        .expect_err("unknown callable function");
    let ReflectErrorKind::UnknownFunction {
        candidates,
        related,
        ..
    } = error.kind
    else {
        panic!("expected unknown function");
    };

    assert_eq!(candidates, vec!["game.reward.grant".to_owned()]);
    assert_eq!(
        related,
        vec![crate::candidates::ReflectCandidate::new(
            "game.reward.grant",
            None
        )]
    );
}

#[test]
fn function_policy_allows_private_functions_with_permissions() {
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game.private_admin").access(
            FunctionAccess::new()
                .public(false)
                .reflect_visible(true)
                .require_permission("game.admin"),
        ),
    );
    let policy = ReflectPolicy::new(
        crate::permissions::ReflectPermissionSet::new()
            .with(crate::permissions::ReflectPermission::AccessPrivate),
    )
    .with_function_permission("game.admin");

    let ReflectValue::Host(HostValue::Record {
        fields: function, ..
    }) = function_with_policy(&registry, "game.private_admin", &policy)
        .expect("private function metadata")
    else {
        panic!("function metadata should be a record");
    };

    assert_eq!(function.get("public"), Some(&HostValue::Bool(false)));
}

#[test]
fn function_call_policy_requires_reflect_callable_metadata() {
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game.inspectable")
            .access(FunctionAccess::new().reflect_visible(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(2), "game.callable")
            .access(FunctionAccess::new().reflect_callable(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game.write_host")
            .effects(FunctionEffectSet {
                reads_host: false,
                writes_host: true,
                emits_events: false,
            })
            .access(FunctionAccess::new().reflect_callable(true)),
    );
    let policy = ReflectPolicy::all();

    let inspectable = registry
        .function_by_name("game.inspectable")
        .expect("inspectable function");
    policy
        .require_function_access(inspectable)
        .expect("visible function can be inspected");
    let error = policy
        .require_function_call_access(inspectable)
        .expect_err("visible function is not callable");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FunctionNotReflectCallable {
            function: "game.inspectable".to_owned()
        }
    );

    let callable = registry
        .function_by_name("game.callable")
        .expect("callable function");
    policy
        .require_function_call_access(callable)
        .expect("callable function");

    let effectful = registry
        .function_by_name("game.write_host")
        .expect("effectful function");
    let read_only_call_policy = ReflectPolicy::new(
        crate::permissions::ReflectPermissionSet::new()
            .with(crate::permissions::ReflectPermission::CallMethods),
    );
    let error = read_only_call_policy
        .require_function_call_access(effectful)
        .expect_err("host-writing function needs effect permission");
    assert_eq!(
        error.kind,
        ReflectErrorKind::FunctionEffectPermissionDenied {
            function: "game.write_host".to_owned(),
            permission: crate::permissions::ReflectPermission::CallHostWriteMethods,
        }
    );
}

#[test]
fn module_exports_with_policy_hide_inaccessible_functions() {
    let mut registry = TypeRegistry::new();
    registry.register_module(ModuleDesc::new("game.reward"));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game.reward.grant").module("game.reward"),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(2), "game.reward.hidden")
            .module("game.reward")
            .access(FunctionAccess::new().reflect_visible(false)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game.reward.private")
            .module("game.reward")
            .access(FunctionAccess::new().public(false).reflect_visible(true)),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(4), "game.reward.admin")
            .module("game.reward")
            .access(FunctionAccess::new().require_permission("game.admin")),
    );

    assert!(has_module_with_policy(
        &registry,
        "game.reward",
        &ReflectPolicy::read_only()
    ));
    assert!(!has_module_with_policy(
        &registry,
        "game.missing",
        &ReflectPolicy::read_only()
    ));
    assert!(has_function_with_policy(
        &registry,
        "game.reward.grant",
        &ReflectPolicy::read_only()
    ));
    assert!(!has_function_with_policy(
        &registry,
        "game.reward.hidden",
        &ReflectPolicy::read_only()
    ));
    assert!(!has_function_with_policy(
        &registry,
        "game.reward.private",
        &ReflectPolicy::read_only()
    ));
    assert!(!has_function_with_policy(
        &registry,
        "game.reward.admin",
        &ReflectPolicy::read_only()
    ));

    assert_eq!(
        exports(&registry, "game.reward").expect("raw exports"),
        ReflectValue::Host(HostValue::Array(vec![
            HostValue::String("game.reward.grant".to_owned()),
            HostValue::String("game.reward.hidden".to_owned()),
            HostValue::String("game.reward.private".to_owned()),
            HostValue::String("game.reward.admin".to_owned()),
        ]))
    );
    let ReflectValue::Host(HostValue::Array(raw_modules)) = modules(&registry) else {
        panic!("raw module list should be an array");
    };
    assert_eq!(raw_modules.len(), 1);
    let ReflectValue::Host(HostValue::Array(raw_functions)) = functions(&registry) else {
        panic!("raw function list should be an array");
    };
    assert_eq!(raw_functions.len(), 4);
    assert_eq!(
        exports_with_policy(&registry, "game.reward", &ReflectPolicy::read_only())
            .expect("policy exports"),
        ReflectValue::Host(HostValue::Array(vec![HostValue::String(
            "game.reward.grant".to_owned()
        )]))
    );
    let ReflectValue::Host(HostValue::Array(policy_functions)) =
        functions_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("policy function list should be an array");
    };
    assert_eq!(policy_functions.len(), 1);
    let ReflectValue::Host(HostValue::Array(policy_modules)) =
        modules_with_policy(&registry, &ReflectPolicy::read_only())
    else {
        panic!("policy module list should be an array");
    };
    let HostValue::Record {
        fields: policy_module,
        ..
    } = &policy_modules[0]
    else {
        panic!("policy module list item should be a record");
    };
    assert_eq!(
        policy_module.get("exports"),
        Some(&HostValue::Array(vec![HostValue::String(
            "game.reward.grant".to_owned()
        )]))
    );

    let ReflectValue::Host(HostValue::Record { fields: module, .. }) =
        module_with_policy(&registry, "game.reward", &ReflectPolicy::read_only())
            .expect("policy module")
    else {
        panic!("module metadata should be a record");
    };
    assert_eq!(
        module.get("exports"),
        Some(&HostValue::Array(vec![HostValue::String(
            "game.reward.grant".to_owned()
        )]))
    );
    assert_eq!(
        exports_for_target_with_policy(
            &registry,
            &ReflectValue::Host(HostValue::Record {
                type_name: "ReflectModule".to_owned(),
                fields: module.clone(),
            }),
            &ReflectPolicy::read_only(),
        )
        .expect("policy module record exports"),
        ReflectValue::Host(HostValue::Array(vec![HostValue::String(
            "game.reward.grant".to_owned()
        )]))
    );

    let admin_policy = ReflectPolicy::new(
        crate::permissions::ReflectPermissionSet::read_only()
            .with(crate::permissions::ReflectPermission::AccessPrivate),
    )
    .with_function_permission("game.admin");
    assert!(has_function_with_policy(
        &registry,
        "game.reward.admin",
        &admin_policy
    ));
    assert_eq!(
        exports_with_policy(&registry, "game.reward", &admin_policy).expect("admin exports"),
        ReflectValue::Host(HostValue::Array(vec![
            HostValue::String("game.reward.grant".to_owned()),
            HostValue::String("game.reward.private".to_owned()),
            HostValue::String("game.reward.admin".to_owned()),
        ]))
    );
}
