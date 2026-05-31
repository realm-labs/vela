use crate::{TypeFact, stdlib::StdlibFunctionFact};

pub(super) fn completion_facts() -> Vec<StdlibFunctionFact> {
    vec![
        fact("reflect.permissions", Vec::new(), array(TypeFact::String)),
        fact(
            "reflect.has_permission",
            vec![TypeFact::String],
            TypeFact::Bool,
        ),
        fact("reflect.type_of", vec![TypeFact::Any], maybe_reflect_type()),
        fact("reflect.types", Vec::new(), array(record("ReflectType"))),
        fact(
            "reflect.type_info",
            vec![TypeFact::String],
            record("ReflectType"),
        ),
        fact("reflect.has_type", vec![TypeFact::String], TypeFact::Bool),
        fact("reflect.name", vec![TypeFact::Any], TypeFact::String),
        fact("reflect.id", vec![TypeFact::Any], TypeFact::Int),
        fact("reflect.kind", vec![TypeFact::Any], TypeFact::String),
        fact("reflect.owner", vec![TypeFact::Any], TypeFact::String),
        fact("reflect.attrs", vec![TypeFact::Any], attrs()),
        fact(
            "reflect.attr",
            vec![TypeFact::Any, TypeFact::String],
            maybe_string(),
        ),
        fact(
            "reflect.has_attr",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Bool,
        ),
        fact("reflect.docs", vec![TypeFact::Any], maybe_string()),
        fact("reflect.origin", vec![TypeFact::Any], maybe_string()),
        fact(
            "reflect.source_span",
            vec![TypeFact::Any],
            maybe_source_span(),
        ),
        fact("reflect.access", vec![TypeFact::Any], access()),
        fact(
            "reflect.required_permissions",
            vec![TypeFact::Any],
            array(TypeFact::String),
        ),
        fact(
            "reflect.effects",
            vec![TypeFact::Any],
            record("ReflectEffectSet"),
        ),
        fact(
            "reflect.params",
            vec![TypeFact::Any],
            array(record("ReflectParam")),
        ),
        fact("reflect.returns", vec![TypeFact::Any], maybe_string()),
        fact("reflect.fields", Vec::new(), array(record("ReflectField"))),
        fact(
            "reflect.fields",
            vec![TypeFact::Any],
            array(record("ReflectField")),
        ),
        fact(
            "reflect.field",
            vec![TypeFact::Any, TypeFact::String],
            record("ReflectField"),
        ),
        fact(
            "reflect.has_field",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Bool,
        ),
        fact(
            "reflect.module",
            vec![TypeFact::String],
            record("ReflectModule"),
        ),
        fact("reflect.has_module", vec![TypeFact::String], TypeFact::Bool),
        fact(
            "reflect.modules",
            Vec::new(),
            array(record("ReflectModule")),
        ),
        fact(
            "reflect.exports",
            vec![module_target()],
            array(TypeFact::String),
        ),
        fact(
            "reflect.function",
            vec![TypeFact::String],
            record("ReflectFunction"),
        ),
        fact(
            "reflect.has_function",
            vec![TypeFact::String],
            TypeFact::Bool,
        ),
        fact(
            "reflect.functions",
            Vec::new(),
            array(record("ReflectFunction")),
        ),
        fact(
            "reflect.methods",
            Vec::new(),
            array(record("ReflectMethod")),
        ),
        fact(
            "reflect.methods",
            vec![TypeFact::Any],
            array(record("ReflectMethod")),
        ),
        fact(
            "reflect.method",
            vec![TypeFact::Any, TypeFact::String],
            record("ReflectMethod"),
        ),
        fact(
            "reflect.has_method",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Bool,
        ),
        fact("reflect.traits", Vec::new(), array(record("ReflectTrait"))),
        fact(
            "reflect.traits",
            vec![TypeFact::Any],
            array(record("ReflectTrait")),
        ),
        fact(
            "reflect.trait_info",
            vec![TypeFact::String],
            record("ReflectTrait"),
        ),
        fact("reflect.has_trait", vec![TypeFact::String], TypeFact::Bool),
        fact(
            "reflect.variants",
            Vec::new(),
            array(record("ReflectVariant")),
        ),
        fact(
            "reflect.variants",
            vec![TypeFact::Any],
            array(record("ReflectVariant")),
        ),
        fact(
            "reflect.variant_info",
            vec![TypeFact::Any, TypeFact::String],
            record("ReflectVariant"),
        ),
        fact(
            "reflect.has_variant",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Bool,
        ),
        fact("reflect.variant", vec![TypeFact::Any], TypeFact::String),
        fact(
            "reflect.variant_is",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Bool,
        ),
        fact(
            "reflect.get",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Any,
        ),
        fact(
            "reflect.set",
            vec![TypeFact::Any, TypeFact::String, TypeFact::Any],
            TypeFact::Any,
        ),
        fact(
            "reflect.call",
            vec![TypeFact::Any, TypeFact::String],
            TypeFact::Any,
        ),
        fact(
            "reflect.implements",
            vec![TypeFact::Any, trait_target()],
            TypeFact::Bool,
        ),
    ]
}

pub(super) fn function_fact(name: &str, args: &[TypeFact]) -> Option<StdlibFunctionFact> {
    let returns = match name {
        "reflect.permissions" | "reflect.types" | "reflect.modules" | "reflect.functions"
            if args.is_empty() =>
        {
            match name {
                "reflect.permissions" => array(TypeFact::String),
                "reflect.types" => array(record("ReflectType")),
                "reflect.modules" => array(record("ReflectModule")),
                "reflect.functions" => array(record("ReflectFunction")),
                _ => unreachable!("name matched above"),
            }
        }
        "reflect.has_permission"
        | "reflect.has_type"
        | "reflect.has_module"
        | "reflect.has_function"
        | "reflect.has_trait"
            if args.len() == 1 =>
        {
            TypeFact::Bool
        }
        "reflect.type_of" if args.len() == 1 => maybe_reflect_type(),
        "reflect.type_info" if args.len() == 1 => record("ReflectType"),
        "reflect.name" | "reflect.kind" | "reflect.variant" if args.len() == 1 => TypeFact::String,
        "reflect.owner" if args.len() == 1 => TypeFact::String,
        "reflect.id" if args.len() == 1 => TypeFact::Int,
        "reflect.attrs" if args.len() == 1 => attrs(),
        "reflect.attr" if args.len() == 2 => maybe_string(),
        "reflect.has_attr" if args.len() == 2 => TypeFact::Bool,
        "reflect.docs" if args.len() == 1 => maybe_string(),
        "reflect.origin" if args.len() == 1 => maybe_string(),
        "reflect.source_span" if args.len() == 1 => maybe_source_span(),
        "reflect.access" if args.len() == 1 => access(),
        "reflect.required_permissions" if args.len() == 1 => array(TypeFact::String),
        "reflect.effects" if args.len() == 1 => record("ReflectEffectSet"),
        "reflect.params" if args.len() == 1 => array(record("ReflectParam")),
        "reflect.returns" if args.len() == 1 => maybe_string(),
        "reflect.fields" => match args.len() {
            0 => array(record("ReflectField")),
            1 => array(record("ReflectField")),
            _ => return None,
        },
        "reflect.field" if args.len() == 2 => record("ReflectField"),
        "reflect.has_field" if args.len() == 2 => TypeFact::Bool,
        "reflect.module" if args.len() == 1 => record("ReflectModule"),
        "reflect.exports" if args.len() == 1 => array(TypeFact::String),
        "reflect.function" if args.len() == 1 => record("ReflectFunction"),
        "reflect.methods" => match args.len() {
            0 | 1 => array(record("ReflectMethod")),
            _ => return None,
        },
        "reflect.method" if args.len() == 2 => record("ReflectMethod"),
        "reflect.has_method" if args.len() == 2 => TypeFact::Bool,
        "reflect.traits" => match args.len() {
            0 | 1 => array(record("ReflectTrait")),
            _ => return None,
        },
        "reflect.trait_info" if args.len() == 1 => record("ReflectTrait"),
        "reflect.variants" => match args.len() {
            0 | 1 => array(record("ReflectVariant")),
            _ => return None,
        },
        "reflect.variant_info" if args.len() == 2 => record("ReflectVariant"),
        "reflect.has_variant" | "reflect.variant_is" if args.len() == 2 => TypeFact::Bool,
        "reflect.get" if args.len() == 2 => TypeFact::Any,
        "reflect.set" if args.len() == 3 => TypeFact::Any,
        "reflect.call" if args.len() >= 2 => TypeFact::Any,
        "reflect.implements" if args.len() == 2 => TypeFact::Bool,
        _ => return None,
    };

    Some(fact(canonical_name(name)?, args.to_vec(), returns))
}

fn fact(name: &'static str, params: Vec<TypeFact>, returns: TypeFact) -> StdlibFunctionFact {
    StdlibFunctionFact::new(name, params, returns)
}

fn array(element: TypeFact) -> TypeFact {
    TypeFact::array(element)
}

fn attrs() -> TypeFact {
    TypeFact::map(TypeFact::String, TypeFact::String)
}

fn trait_target() -> TypeFact {
    TypeFact::union([TypeFact::String, record("ReflectTrait")])
}

fn access() -> TypeFact {
    TypeFact::union([
        record("ReflectFieldAccess"),
        record("ReflectMethodAccess"),
        record("ReflectFunctionAccess"),
    ])
}

fn maybe_string() -> TypeFact {
    TypeFact::union([TypeFact::String, TypeFact::Null])
}

fn maybe_reflect_type() -> TypeFact {
    TypeFact::union([record("ReflectType"), TypeFact::Null])
}

fn maybe_source_span() -> TypeFact {
    TypeFact::union([record("ReflectSourceSpan"), TypeFact::Null])
}

fn module_target() -> TypeFact {
    TypeFact::union([TypeFact::String, record("ReflectModule")])
}

fn record(name: &'static str) -> TypeFact {
    TypeFact::record(name)
}

fn canonical_name(name: &str) -> Option<&'static str> {
    match name {
        "reflect.permissions" => Some("reflect.permissions"),
        "reflect.has_permission" => Some("reflect.has_permission"),
        "reflect.type_of" => Some("reflect.type_of"),
        "reflect.types" => Some("reflect.types"),
        "reflect.type_info" => Some("reflect.type_info"),
        "reflect.has_type" => Some("reflect.has_type"),
        "reflect.name" => Some("reflect.name"),
        "reflect.id" => Some("reflect.id"),
        "reflect.kind" => Some("reflect.kind"),
        "reflect.owner" => Some("reflect.owner"),
        "reflect.attrs" => Some("reflect.attrs"),
        "reflect.attr" => Some("reflect.attr"),
        "reflect.has_attr" => Some("reflect.has_attr"),
        "reflect.docs" => Some("reflect.docs"),
        "reflect.origin" => Some("reflect.origin"),
        "reflect.source_span" => Some("reflect.source_span"),
        "reflect.access" => Some("reflect.access"),
        "reflect.required_permissions" => Some("reflect.required_permissions"),
        "reflect.effects" => Some("reflect.effects"),
        "reflect.params" => Some("reflect.params"),
        "reflect.returns" => Some("reflect.returns"),
        "reflect.fields" => Some("reflect.fields"),
        "reflect.field" => Some("reflect.field"),
        "reflect.has_field" => Some("reflect.has_field"),
        "reflect.module" => Some("reflect.module"),
        "reflect.has_module" => Some("reflect.has_module"),
        "reflect.modules" => Some("reflect.modules"),
        "reflect.exports" => Some("reflect.exports"),
        "reflect.function" => Some("reflect.function"),
        "reflect.has_function" => Some("reflect.has_function"),
        "reflect.functions" => Some("reflect.functions"),
        "reflect.methods" => Some("reflect.methods"),
        "reflect.method" => Some("reflect.method"),
        "reflect.has_method" => Some("reflect.has_method"),
        "reflect.traits" => Some("reflect.traits"),
        "reflect.trait_info" => Some("reflect.trait_info"),
        "reflect.has_trait" => Some("reflect.has_trait"),
        "reflect.variants" => Some("reflect.variants"),
        "reflect.variant_info" => Some("reflect.variant_info"),
        "reflect.has_variant" => Some("reflect.has_variant"),
        "reflect.variant" => Some("reflect.variant"),
        "reflect.variant_is" => Some("reflect.variant_is"),
        "reflect.get" => Some("reflect.get"),
        "reflect.set" => Some("reflect.set"),
        "reflect.call" => Some("reflect.call"),
        "reflect.implements" => Some("reflect.implements"),
        _ => None,
    }
}
