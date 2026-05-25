use vela_reflect::{
    DeclOrigin, FieldDesc, FunctionDesc, FunctionEffectSet, MethodDesc, MethodEffectSet,
    ModuleDesc, TraitDesc, TraitMethodDesc, TypeKind, VariantDesc,
};

pub(super) fn type_detail(kind: TypeKind) -> String {
    match kind {
        TypeKind::Host => "kind: host".to_owned(),
        TypeKind::ScriptStruct => "kind: script struct".to_owned(),
        TypeKind::ScriptEnum => "kind: script enum".to_owned(),
    }
}

pub(super) fn field_detail(desc: &FieldDesc) -> String {
    let permissions = permission_detail(desc.access.required_permissions());
    format!(
        "writable: {}; reflect_readable: {}; reflect_writable: {}; permissions: {permissions}",
        desc.writable, desc.access.reflect_readable, desc.access.reflect_writable
    )
}

pub(super) fn method_detail(desc: &MethodDesc) -> String {
    format!(
        "{}; access: {}; permissions: {}",
        method_effect_detail(&desc.effects),
        if desc.access.public {
            "public"
        } else {
            "private"
        },
        permission_detail(desc.access.required_permissions())
    )
}

pub(super) fn function_detail(desc: &FunctionDesc) -> String {
    format!(
        "origin: {}; {}; access: {}; permissions: {}",
        origin_detail(desc.origin),
        function_effect_detail(&desc.effects),
        if desc.access.public {
            "public"
        } else {
            "private"
        },
        permission_detail(desc.access.required_permissions())
    )
}

pub(super) fn trait_detail(desc: &TraitDesc) -> String {
    format!("methods: {}", desc.methods.len())
}

pub(super) fn trait_method_detail(desc: &TraitMethodDesc) -> String {
    format!("defaulted: {}", desc.has_default)
}

pub(super) fn variant_detail(desc: &VariantDesc) -> String {
    format!("fields: {}", desc.fields.len())
}

pub(super) fn module_detail(desc: &ModuleDesc) -> String {
    format!("exports: {}", desc.exports.len())
}

fn function_effect_detail(effects: &FunctionEffectSet) -> String {
    effect_detail(
        effects.reads_host,
        effects.writes_host,
        effects.emits_events,
    )
}

fn method_effect_detail(effects: &MethodEffectSet) -> String {
    effect_detail(
        effects.reads_host,
        effects.writes_host,
        effects.emits_events,
    )
}

fn effect_detail(reads_host: bool, writes_host: bool, emits_events: bool) -> String {
    let mut effects = Vec::new();
    if reads_host {
        effects.push("reads_host");
    }
    if writes_host {
        effects.push("writes_host");
    }
    if emits_events {
        effects.push("emits_events");
    }
    if effects.is_empty() {
        "effects: pure".to_owned()
    } else {
        format!("effects: {}", effects.join(", "))
    }
}

fn origin_detail(origin: DeclOrigin) -> &'static str {
    match origin {
        DeclOrigin::Host => "host",
        DeclOrigin::Script => "script",
    }
}

fn permission_detail(permissions: &[String]) -> String {
    if permissions.is_empty() {
        "none".to_owned()
    } else {
        permissions.join(", ")
    }
}
