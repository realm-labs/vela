use vela_analysis::completion::{CompletionItem, CompletionKind, member_completions};
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::{HostMethodId, SourceId};
use vela_def::{FieldId, TypeId};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TypeDesc, TypeKey, TypeRegistry,
};
use vela_syntax::parser::parse_source;

const HOST_MEMBER_SOURCE: &str =
    include_str!("../../../tests/fixtures/completion/host_member.vela");
const HOST_MEMBER_EXPECTED: &str =
    include_str!("../../../tests/fixtures/completion/host_member.expected");

#[test]
fn host_member_completion_fixture_suggests_registry_fields_and_methods() {
    let parsed = parse_source(SourceId::new(1), &normalized_fixture(HOST_MEMBER_SOURCE));
    assert_eq!(parsed.diagnostics, []);

    let mut completions = member_completions(&registry_facts(), &TypeFact::host("Player"));
    completions.sort_by(|left, right| {
        completion_rank(&left.kind)
            .cmp(&completion_rank(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
    });
    let rendered = completions
        .iter()
        .map(render_completion)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        rendered.trim_end(),
        normalized_fixture(HOST_MEMBER_EXPECTED).trim_end()
    );
}

fn normalized_fixture(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn render_completion(completion: &CompletionItem) -> String {
    format!(
        "{} {} {}",
        completion_kind_name(&completion.kind),
        completion.label,
        completion.fact.display_name()
    )
}

fn completion_kind_name(kind: &CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Binding => "binding",
        CompletionKind::Const => "const",
        CompletionKind::Field => "field",
        CompletionKind::Method => "method",
        CompletionKind::Module => "module",
        CompletionKind::Variant => "variant",
        CompletionKind::Function => "function",
        CompletionKind::Type => "type",
        CompletionKind::Trait => "trait",
    }
}

fn completion_rank(kind: &CompletionKind) -> u8 {
    match kind {
        CompletionKind::Field => 0,
        CompletionKind::Method => 1,
        CompletionKind::Binding => 2,
        CompletionKind::Const => 3,
        CompletionKind::Function => 4,
        CompletionKind::Module => 5,
        CompletionKind::Type => 6,
        CompletionKind::Trait => 7,
        CompletionKind::Variant => 8,
    }
}

fn registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("int"))
            .field(FieldDesc::new(FieldId::new(2), "inventory").type_hint("map"))
            .method(
                MethodDesc::new(HostMethodId::new(1), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("bool"),
            ),
    );
    RegistryFacts::from_registry(&registry)
}
