use std::sync::Arc;

use vela_host::ScriptStateAdapter;
use vela_reflect::{self as reflect, TypeRegistry};

use crate::{
    Value, Vm, VmError, VmErrorKind, VmResult, expect_arity, expect_string, value_from_reflect,
    value_to_reflect,
};

impl Vm {
    pub fn register_reflection_natives(&mut self, registry: Arc<TypeRegistry>) {
        self.register_reflection_natives_with_policy(registry, reflect::ReflectPolicy::all());
    }

    pub fn register_reflection_natives_with_permissions(
        &mut self,
        registry: Arc<TypeRegistry>,
        permissions: reflect::ReflectPermissionSet,
    ) {
        self.register_reflection_natives_with_policy(
            registry,
            reflect::ReflectPolicy::new(permissions),
        );
    }

    pub fn register_reflection_natives_with_policy(
        &mut self,
        registry: Arc<TypeRegistry>,
        policy: reflect::ReflectPolicy,
    ) {
        self.register_type_registry(Arc::clone(&registry));
        let lookup_budget = Arc::new(reflect::ReflectLookupBudget::new(policy.lookup_limit()));
        let type_of_registry = Arc::clone(&registry);
        let type_of_policy = policy.clone();
        let type_of_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.type_of", move |args, _host| {
            check_reflect_policy(
                &type_of_policy,
                &type_of_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.type_of", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.type_of")?;
            check_host_ref_inspection(&type_of_policy, &target)?;
            Ok(reflect::type_of(&type_of_registry, &target)
                .map_or(Value::Null, |desc| Value::String(desc.key.name.clone())))
        });

        let name_registry = Arc::clone(&registry);
        let name_policy = policy.clone();
        let name_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.name", move |args, _host| {
            check_reflect_policy(
                &name_policy,
                &name_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.name", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.name")?;
            check_host_ref_inspection(&name_policy, &target)?;
            value_from_reflect(reflect::name_metadata(&name_registry, &target)?)
        });

        let kind_registry = Arc::clone(&registry);
        let kind_policy = policy.clone();
        let kind_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.kind", move |args, _host| {
            check_reflect_policy(
                &kind_policy,
                &kind_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.kind", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.kind")?;
            check_host_ref_inspection(&kind_policy, &target)?;
            value_from_reflect(reflect::kind_metadata(&kind_registry, &target)?)
        });

        let attrs_registry = Arc::clone(&registry);
        let attrs_policy = policy.clone();
        let attrs_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.attrs", move |args, _host| {
            check_reflect_policy(
                &attrs_policy,
                &attrs_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.attrs", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.attrs")?;
            check_host_ref_inspection(&attrs_policy, &target)?;
            value_from_reflect(reflect::attrs_metadata(&attrs_registry, &target)?)
        });

        let docs_registry = Arc::clone(&registry);
        let docs_policy = policy.clone();
        let docs_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.docs", move |args, _host| {
            check_reflect_policy(
                &docs_policy,
                &docs_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.docs", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.docs")?;
            check_host_ref_inspection(&docs_policy, &target)?;
            value_from_reflect(reflect::docs_metadata(&docs_registry, &target)?)
        });

        let fields_registry = Arc::clone(&registry);
        let fields_policy = policy.clone();
        let fields_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.fields", move |args, _host| {
            check_reflect_policy(
                &fields_policy,
                &fields_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.fields", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.fields")?;
            check_host_ref_inspection(&fields_policy, &target)?;
            let Some(desc) = reflect::type_of(&fields_registry, &target) else {
                return Ok(Value::Null);
            };
            let fields = reflect::fields(&fields_registry, &desc.key)
                .unwrap_or(&[])
                .iter()
                .map(|field| Value::String(field.name.clone()))
                .collect();
            Ok(Value::Array(fields))
        });

        let field_registry = Arc::clone(&registry);
        let field_policy = policy.clone();
        let field_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.field", move |args, _host| {
            check_reflect_policy(
                &field_policy,
                &field_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.field", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.field")?;
            check_host_ref_inspection(&field_policy, &target)?;
            let field_name = expect_string(&args[1], "reflect.field")?;
            value_from_reflect(reflect::field_metadata(
                &field_registry,
                &target,
                field_name,
            )?)
        });

        let has_field_registry = Arc::clone(&registry);
        let has_field_policy = policy.clone();
        let has_field_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.has_field", move |args, _host| {
            check_reflect_policy(
                &has_field_policy,
                &has_field_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.has_field", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.has_field")?;
            check_host_ref_inspection(&has_field_policy, &target)?;
            let field_name = expect_string(&args[1], "reflect.has_field")?;
            Ok(Value::Bool(reflect::has_field(
                &has_field_registry,
                &target,
                field_name,
            )?))
        });

        let module_registry = Arc::clone(&registry);
        let module_policy = policy.clone();
        let module_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.module", move |args, _host| {
            check_reflect_policy(
                &module_policy,
                &module_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.module", args, 1)?;
            let module_name = expect_string(&args[0], "reflect.module")?;
            value_from_reflect(reflect::module_metadata(&module_registry, module_name)?)
        });

        let exports_registry = Arc::clone(&registry);
        let exports_policy = policy.clone();
        let exports_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.exports", move |args, _host| {
            check_reflect_policy(
                &exports_policy,
                &exports_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.exports", args, 1)?;
            let module_name = expect_string(&args[0], "reflect.exports")?;
            value_from_reflect(reflect::module_exports(&exports_registry, module_name)?)
        });

        let function_registry = Arc::clone(&registry);
        let function_policy = policy.clone();
        let function_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.function", move |args, _host| {
            check_reflect_policy(
                &function_policy,
                &function_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.function", args, 1)?;
            let function_name = expect_string(&args[0], "reflect.function")?;
            value_from_reflect(reflect::function_metadata(
                &function_registry,
                function_name,
            )?)
        });

        let methods_registry = Arc::clone(&registry);
        let methods_policy = policy.clone();
        let methods_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.methods", move |args, _host| {
            check_reflect_policy(
                &methods_policy,
                &methods_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.methods", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.methods")?;
            check_host_ref_inspection(&methods_policy, &target)?;
            value_from_reflect(reflect::methods(&methods_registry, &target)?)
        });

        let has_method_registry = Arc::clone(&registry);
        let has_method_policy = policy.clone();
        let has_method_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.has_method", move |args, _host| {
            check_reflect_policy(
                &has_method_policy,
                &has_method_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.has_method", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.has_method")?;
            check_host_ref_inspection(&has_method_policy, &target)?;
            let method_name = expect_string(&args[1], "reflect.has_method")?;
            Ok(Value::Bool(reflect::has_method(
                &has_method_registry,
                &target,
                method_name,
            )?))
        });

        let traits_registry = Arc::clone(&registry);
        let traits_policy = policy.clone();
        let traits_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.traits", move |args, _host| {
            check_reflect_policy(
                &traits_policy,
                &traits_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.traits", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.traits")?;
            check_host_ref_inspection(&traits_policy, &target)?;
            value_from_reflect(reflect::trait_metadata(&traits_registry, &target)?)
        });

        let variants_registry = Arc::clone(&registry);
        let variants_policy = policy.clone();
        let variants_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.variants", move |args, _host| {
            check_reflect_policy(
                &variants_policy,
                &variants_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.variants", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.variants")?;
            check_host_ref_inspection(&variants_policy, &target)?;
            value_from_reflect(reflect::variant_metadata(&variants_registry, &target)?)
        });

        let variant_policy = policy.clone();
        let variant_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.variant", move |args, _host| {
            check_reflect_policy(
                &variant_policy,
                &variant_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.variant", args, 1)?;
            let target = value_to_reflect(&args[0], "reflect.variant")?;
            check_host_ref_inspection(&variant_policy, &target)?;
            value_from_reflect(reflect::variant(&target)?)
        });

        let variant_is_policy = policy.clone();
        let variant_is_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.variant_is", move |args, _host| {
            check_reflect_policy(
                &variant_is_policy,
                &variant_is_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.variant_is", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.variant_is")?;
            check_host_ref_inspection(&variant_is_policy, &target)?;
            let variant_name = expect_string(&args[1], "reflect.variant_is")?;
            Ok(Value::Bool(reflect::variant_is(&target, variant_name)?))
        });

        let get_registry = Arc::clone(&registry);
        let get_policy = policy.clone();
        let get_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.get", move |args, host| {
            check_reflect_policy(
                &get_policy,
                &get_budget,
                reflect::ReflectPermission::ReadValueFields,
            )?;
            expect_arity("reflect.get", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.get")?;
            let field = expect_string(&args[1], "reflect.get")?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &get_registry,
                adapter,
                tx: &mut *host.tx,
            };
            let value = reflect::get(&mut ctx, &target, field)?;
            value_from_reflect(value)
        });

        let set_registry = Arc::clone(&registry);
        let set_policy = policy.clone();
        let set_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.set", move |args, host| {
            check_reflect_policy(
                &set_policy,
                &set_budget,
                reflect::ReflectPermission::WriteValueFields,
            )?;
            expect_arity("reflect.set", args, 3)?;
            let target = value_to_reflect(&args[0], "reflect.set")?;
            let field = expect_string(&args[1], "reflect.set")?;
            let value = value_to_reflect(&args[2], "reflect.set")?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &set_registry,
                adapter,
                tx: &mut *host.tx,
            };
            reflect::set(&mut ctx, &target, field, value)?;
            Ok(Value::Null)
        });

        let call_registry = Arc::clone(&registry);
        let call_policy = policy.clone();
        let call_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.call", move |args, host| {
            check_reflect_policy(
                &call_policy,
                &call_budget,
                reflect::ReflectPermission::CallMethods,
            )?;
            if args.len() < 2 {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: "reflect.call".to_owned(),
                    expected: 2,
                    actual: args.len(),
                }));
            }
            let target = value_to_reflect(&args[0], "reflect.call")?;
            let method = expect_string(&args[1], "reflect.call")?;
            let call_args = args[2..]
                .iter()
                .map(|arg| value_to_reflect(arg, "reflect.call"))
                .collect::<VmResult<Vec<_>>>()?;
            let adapter: &dyn ScriptStateAdapter = &*host.adapter;
            let mut ctx = reflect::ReflectContext {
                registry: &call_registry,
                adapter,
                tx: &mut *host.tx,
            };
            let value =
                reflect::call_with_policy(&mut ctx, &target, method, call_args, &call_policy)?;
            value_from_reflect(value)
        });

        let implements_policy = policy;
        let implements_budget = Arc::clone(&lookup_budget);
        self.register_host_native("reflect.implements", move |args, _host| {
            check_reflect_policy(
                &implements_policy,
                &implements_budget,
                reflect::ReflectPermission::ReadTypeInfo,
            )?;
            expect_arity("reflect.implements", args, 2)?;
            let target = value_to_reflect(&args[0], "reflect.implements")?;
            check_host_ref_inspection(&implements_policy, &target)?;
            let trait_name = expect_string(&args[1], "reflect.implements")?;
            Ok(Value::Bool(reflect::implements(
                &registry, &target, trait_name,
            )?))
        });
    }
}

fn check_reflect_policy(
    policy: &reflect::ReflectPolicy,
    lookup_budget: &reflect::ReflectLookupBudget,
    permission: reflect::ReflectPermission,
) -> VmResult<()> {
    policy.require(permission)?;
    lookup_budget.consume()?;
    Ok(())
}

fn check_host_ref_inspection(
    policy: &reflect::ReflectPolicy,
    target: &reflect::ReflectValue,
) -> VmResult<()> {
    if matches!(target, reflect::ReflectValue::HostRef(_)) {
        policy.require(reflect::ReflectPermission::InspectHostPath)?;
    }
    Ok(())
}
