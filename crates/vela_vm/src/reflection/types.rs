use std::sync::Arc;

use vela_reflect::registry::TypeRegistry;
use vela_reflect::{self as reflect};

use crate::{Vm, expect_arity, expect_string, value_from_reflect, value_to_reflect};

use super::common::{check_host_ref_inspection, check_reflect_policy};

pub(super) fn register(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let type_of_registry = Arc::clone(registry);
    let type_of_policy = policy.clone();
    let type_of_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.type_of", move |args, _host| {
        check_reflect_policy(
            &type_of_policy,
            &type_of_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.type_of", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.type_of")?;
        check_host_ref_inspection(&type_of_policy, &target)?;
        value_from_reflect(reflect::types::type_of_value(&type_of_registry, &target))
    });

    let types_registry = Arc::clone(registry);
    let types_policy = policy.clone();
    let types_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.types", move |args, _host| {
        check_reflect_policy(
            &types_policy,
            &types_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.types", args, 0)?;
        value_from_reflect(reflect::types::type_list(&types_registry))
    });

    let type_info_registry = Arc::clone(registry);
    let type_info_policy = policy.clone();
    let type_info_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.type_info", move |args, _host| {
        check_reflect_policy(
            &type_info_policy,
            &type_info_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.type_info", args, 1)?;
        let type_name = expect_string(&args[0], "reflect.type_info")?;
        value_from_reflect(reflect::types::type_by_name(
            &type_info_registry,
            type_name,
        )?)
    });

    let has_type_registry = Arc::clone(registry);
    let has_type_policy = policy.clone();
    let has_type_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_type", move |args, _host| {
        check_reflect_policy(
            &has_type_policy,
            &has_type_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_type", args, 1)?;
        let type_name = expect_string(&args[0], "reflect.has_type")?;
        Ok(crate::Value::Bool(reflect::types::has_type(
            &has_type_registry,
            type_name,
        )))
    });

    register_named_metadata(vm, registry, policy, lookup_budget);
}

fn register_named_metadata(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let name_registry = Arc::clone(registry);
    let name_policy = policy.clone();
    let name_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.name", move |args, _host| {
        check_reflect_policy(
            &name_policy,
            &name_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.name", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.name")?;
        check_host_ref_inspection(&name_policy, &target)?;
        value_from_reflect(reflect::members::name(&name_registry, &target)?)
    });

    let id_registry = Arc::clone(registry);
    let id_policy = policy.clone();
    let id_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.id", move |args, _host| {
        check_reflect_policy(
            &id_policy,
            &id_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.id", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.id")?;
        check_host_ref_inspection(&id_policy, &target)?;
        value_from_reflect(reflect::members::id(&id_registry, &target)?)
    });

    let kind_registry = Arc::clone(registry);
    let kind_policy = policy.clone();
    let kind_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.kind", move |args, _host| {
        check_reflect_policy(
            &kind_policy,
            &kind_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.kind", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.kind")?;
        check_host_ref_inspection(&kind_policy, &target)?;
        value_from_reflect(reflect::members::kind(&kind_registry, &target)?)
    });

    let owner_registry = Arc::clone(registry);
    let owner_policy = policy.clone();
    let owner_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.owner", move |args, _host| {
        check_reflect_policy(
            &owner_policy,
            &owner_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.owner", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.owner")?;
        check_host_ref_inspection(&owner_policy, &target)?;
        value_from_reflect(reflect::members::owner(&owner_registry, &target)?)
    });

    register_attribute_metadata(vm, registry, policy, lookup_budget);
    register_signature_metadata(vm, registry, policy, lookup_budget);
}

fn register_attribute_metadata(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let attrs_registry = Arc::clone(registry);
    let attrs_policy = policy.clone();
    let attrs_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.attrs", move |args, _host| {
        check_reflect_policy(
            &attrs_policy,
            &attrs_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.attrs", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.attrs")?;
        check_host_ref_inspection(&attrs_policy, &target)?;
        value_from_reflect(reflect::members::attrs(&attrs_registry, &target)?)
    });

    let attr_registry = Arc::clone(registry);
    let attr_policy = policy.clone();
    let attr_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.attr", move |args, _host| {
        check_reflect_policy(
            &attr_policy,
            &attr_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.attr", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.attr")?;
        check_host_ref_inspection(&attr_policy, &target)?;
        let name = expect_string(&args[1], "reflect.attr")?;
        value_from_reflect(reflect::members::attr(&attr_registry, &target, name)?)
    });

    let has_attr_registry = Arc::clone(registry);
    let has_attr_policy = policy.clone();
    let has_attr_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.has_attr", move |args, _host| {
        check_reflect_policy(
            &has_attr_policy,
            &has_attr_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.has_attr", args, 2)?;
        let target = value_to_reflect(&args[0], "reflect.has_attr")?;
        check_host_ref_inspection(&has_attr_policy, &target)?;
        let name = expect_string(&args[1], "reflect.has_attr")?;
        Ok(crate::Value::Bool(reflect::members::has_attr(
            &has_attr_registry,
            &target,
            name,
        )?))
    });

    let docs_registry = Arc::clone(registry);
    let docs_policy = policy.clone();
    let docs_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.docs", move |args, _host| {
        check_reflect_policy(
            &docs_policy,
            &docs_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.docs", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.docs")?;
        check_host_ref_inspection(&docs_policy, &target)?;
        value_from_reflect(reflect::members::docs(&docs_registry, &target)?)
    });

    let origin_registry = Arc::clone(registry);
    let origin_policy = policy.clone();
    let origin_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.origin", move |args, _host| {
        check_reflect_policy(
            &origin_policy,
            &origin_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.origin", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.origin")?;
        check_host_ref_inspection(&origin_policy, &target)?;
        value_from_reflect(reflect::members::origin(&origin_registry, &target)?)
    });

    let source_span_registry = Arc::clone(registry);
    let source_span_policy = policy.clone();
    let source_span_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.source_span", move |args, _host| {
        check_reflect_policy(
            &source_span_policy,
            &source_span_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.source_span", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.source_span")?;
        check_host_ref_inspection(&source_span_policy, &target)?;
        value_from_reflect(reflect::members::source_span(
            &source_span_registry,
            &target,
        )?)
    });
}

fn register_signature_metadata(
    vm: &mut Vm,
    registry: &Arc<TypeRegistry>,
    policy: &reflect::permissions::ReflectPolicy,
    lookup_budget: &Arc<reflect::permissions::ReflectLookupBudget>,
) {
    let access_registry = Arc::clone(registry);
    let access_policy = policy.clone();
    let access_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.access", move |args, _host| {
        check_reflect_policy(
            &access_policy,
            &access_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.access", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.access")?;
        check_host_ref_inspection(&access_policy, &target)?;
        value_from_reflect(reflect::members::access(&access_registry, &target)?)
    });

    let required_permissions_registry = Arc::clone(registry);
    let required_permissions_policy = policy.clone();
    let required_permissions_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.required_permissions", move |args, _host| {
        check_reflect_policy(
            &required_permissions_policy,
            &required_permissions_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.required_permissions", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.required_permissions")?;
        check_host_ref_inspection(&required_permissions_policy, &target)?;
        value_from_reflect(reflect::members::required_permissions(
            &required_permissions_registry,
            &target,
        )?)
    });

    let effects_registry = Arc::clone(registry);
    let effects_policy = policy.clone();
    let effects_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.effects", move |args, _host| {
        check_reflect_policy(
            &effects_policy,
            &effects_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.effects", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.effects")?;
        check_host_ref_inspection(&effects_policy, &target)?;
        value_from_reflect(reflect::members::effects(&effects_registry, &target)?)
    });

    let params_registry = Arc::clone(registry);
    let params_policy = policy.clone();
    let params_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.params", move |args, _host| {
        check_reflect_policy(
            &params_policy,
            &params_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.params", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.params")?;
        check_host_ref_inspection(&params_policy, &target)?;
        value_from_reflect(reflect::members::params(&params_registry, &target)?)
    });

    let returns_registry = Arc::clone(registry);
    let returns_policy = policy.clone();
    let returns_budget = Arc::clone(lookup_budget);
    vm.register_host_native("reflect.returns", move |args, _host| {
        check_reflect_policy(
            &returns_policy,
            &returns_budget,
            reflect::permissions::ReflectPermission::ReadTypeInfo,
        )?;
        expect_arity("reflect.returns", args, 1)?;
        let target = value_to_reflect(&args[0], "reflect.returns")?;
        check_host_ref_inspection(&returns_policy, &target)?;
        value_from_reflect(reflect::members::returns(&returns_registry, &target)?)
    });
}
