use super::ServiceSchemaError;

pub(super) fn valid_service_member_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

pub(super) const fn service_compile_effect(
    effect: crate::native::EffectSet,
) -> vela_mir::MirEffect {
    vela_mir::MirEffect {
        host_read: effect.reads_host(),
        host_write: effect.writes_host(),
        reflection_read: effect.reads_reflection(),
        reflection_write: effect.writes_reflection(),
        reflection_call: effect.calls_reflection(),
        emits_event: effect.emits_events(),
        reads_time: effect.reads_time(),
        uses_random: effect.uses_random(),
        reads_io: effect.reads_io(),
        writes_io: effect.writes_io(),
        ..vela_mir::MirEffect::PURE
    }
}

#[derive(Clone, Copy)]
pub(super) enum ServicePathKind {
    Service,
    ServiceSet,
}

pub(super) fn validate_qualified_path(
    path: &str,
    kind: ServicePathKind,
) -> Result<(), ServiceSchemaError> {
    if !path.is_empty() && path.split("::").all(is_simple_identifier) {
        return Ok(());
    }
    match kind {
        ServicePathKind::Service => Err(ServiceSchemaError::InvalidServicePath(path.to_owned())),
        ServicePathKind::ServiceSet => {
            Err(ServiceSchemaError::InvalidServiceSetPath(path.to_owned()))
        }
    }
}

pub(super) fn is_simple_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
