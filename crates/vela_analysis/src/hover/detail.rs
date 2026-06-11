use vela_reflect::access::{FunctionEffectSet, MethodEffectSet};
use vela_reflect::modules::{DeclOrigin, FunctionDesc, ModuleDesc};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, TraitDesc, TraitMethodDesc, TypeKind, VariantDesc,
};

pub(super) fn type_detail(kind: TypeKind) -> String {
    match kind {
        TypeKind::Null => "kind: null".to_owned(),
        TypeKind::Bool => "kind: bool".to_owned(),
        TypeKind::Int => "kind: int".to_owned(),
        TypeKind::Float => "kind: float".to_owned(),
        TypeKind::String => "kind: string".to_owned(),
        TypeKind::Bytes => "kind: bytes".to_owned(),
        TypeKind::Array => "kind: array".to_owned(),
        TypeKind::Map => "kind: map".to_owned(),
        TypeKind::Set => "kind: set".to_owned(),
        TypeKind::Range => "kind: range".to_owned(),
        TypeKind::Function => "kind: function".to_owned(),
        TypeKind::Closure => "kind: closure".to_owned(),
        TypeKind::Host => "kind: host".to_owned(),
        TypeKind::ScriptStruct => "kind: script struct".to_owned(),
        TypeKind::ScriptEnum => "kind: script enum".to_owned(),
    }
}

pub(super) fn field_detail(desc: &FieldDesc) -> String {
    let permissions = permission_detail(desc.access.required_permissions());
    format!(
        "writable: {}; reflect_readable: {}; reflect_writable: {}; reflection permissions: {permissions}",
        desc.writable, desc.access.reflect_readable, desc.access.reflect_writable
    )
}

pub(super) fn method_detail(desc: &MethodDesc) -> String {
    format!(
        "{}; access: {}; reflection permissions: {}",
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
        "origin: {}; {}; access: {}; capabilities: {}",
        origin_detail(desc.origin),
        function_effect_detail(&desc.effects),
        if desc.access.public {
            "public"
        } else {
            "private"
        },
        function_capability_detail(&desc.effects)
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
    effect_detail([
        ("reads_host", effects.reads_host),
        ("writes_host", effects.writes_host),
        ("emits_events", effects.emits_events),
        ("reads_time", effects.reads_time),
        ("uses_random", effects.uses_random),
        ("reads_io", effects.reads_io),
        ("writes_io", effects.writes_io),
        ("reads_reflection", effects.reads_reflection),
        ("writes_reflection", effects.writes_reflection),
        ("calls_reflection", effects.calls_reflection),
    ])
}

fn method_effect_detail(effects: &MethodEffectSet) -> String {
    effect_detail([
        ("reads_host", effects.reads_host),
        ("writes_host", effects.writes_host),
        ("emits_events", effects.emits_events),
        ("reads_time", effects.reads_time),
        ("uses_random", effects.uses_random),
        ("reads_io", effects.reads_io),
        ("writes_io", effects.writes_io),
        ("reads_reflection", effects.reads_reflection),
        ("writes_reflection", effects.writes_reflection),
        ("calls_reflection", effects.calls_reflection),
    ])
}

fn effect_detail<const N: usize>(items: [(&'static str, bool); N]) -> String {
    let mut effects = Vec::new();
    for (name, enabled) in items {
        if enabled {
            effects.push(name);
        }
    }
    if effects.is_empty() {
        "effects: pure".to_owned()
    } else {
        format!("effects: {}", effects.join(", "))
    }
}

fn function_capability_detail(effects: &FunctionEffectSet) -> String {
    let mut capabilities = Vec::new();
    if effects.reads_host {
        capabilities.push("host_read");
    }
    if effects.writes_host {
        capabilities.push("host_write");
    }
    if effects.emits_events {
        capabilities.push("event_emit");
    }
    if effects.reads_time {
        capabilities.push("time");
    }
    if effects.uses_random {
        capabilities.push("random");
    }
    if effects.reads_io {
        capabilities.push("io_read");
    }
    if effects.writes_io {
        capabilities.push("io_write");
    }
    if effects.reads_reflection {
        capabilities.push("reflection_read");
    }
    if effects.writes_reflection {
        capabilities.push("reflection_write");
    }
    if effects.calls_reflection {
        capabilities.push("reflection_call");
    }
    if capabilities.is_empty() {
        "none".to_owned()
    } else {
        capabilities.join(", ")
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
