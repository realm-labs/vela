use super::*;
use vela_common::HostTypeId;
use vela_def::{DefPath, FieldId};
use vela_host::target::HostPathPart;

#[test]
fn compiler_lowers_host_field_parameter_defaults_from_cst() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(3)
            .type_hint(Some("i64".to_owned())),
        )
        .expect("host field should register");

    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn grant(player: Player, value = player.level) {
    return value;
}

fn main(player: Player) {
    return grant(player);
}
"#,
        registry.compile_view(),
    )
    .expect("CST-backed host field parameter default should compile");
    let grant = program.function("grant").expect("grant function");
    let target = grant
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => Some(target),
            _ => None,
        })
        .expect("parameter default should emit a host read");
    let plan = grant.host_target(target).expect("host target should exist");

    assert_eq!(plan.root_type, HostTypeId::new(77));
    assert_eq!(
        plan.parts.as_slice(),
        [HostPathPart::Field(FieldId::new(3))]
    );
}
