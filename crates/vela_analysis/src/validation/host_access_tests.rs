use vela_common::{HostTypeId, SourceId, Span};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirDeclId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::callable::{CallableParameterFact, CallableSignatureFact};
use crate::executable::{
    ExecutableAnalysisGeneration, ExecutableAnalysisInput, ExecutableReceiverInput,
};
use crate::registry::{
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryFieldTargetFact,
    RegistryIndexCapabilityFact, RegistryTypeTargetFact,
};
use crate::type_fact::TypeFact;

use super::{HostAccessUseKind, HostIndexCapabilityResolutionFact};

#[test]
fn host_access_diagnostics_freeze_codes_messages_spans_and_labels() {
    let cases = [
        DiagnosticCase {
            source: "fn main(player: Player) { return player.inventory[\"gold\"]; }",
            capability: None,
            code: "analysis::host_index_not_supported",
            message: "type `Inventory` does not support host index access",
            primary: "player.inventory[\"gold\"]",
            labels: &[
                (
                    "player.inventory[\"gold\"]",
                    "host index access is not registered for this type",
                ),
                (
                    "player.inventory",
                    "register a host index capability or expose a field/method instead",
                ),
            ],
        },
        DiagnosticCase {
            source: "fn main(player: Player) { return player.inventory[\"gold\"]; }",
            capability: Some(index_capability(
                false,
                false,
                false,
                false,
                TypeFact::STRING,
            )),
            code: "analysis::host_index_not_readable",
            message: "type `Inventory` does not allow host index reads",
            primary: "player.inventory[\"gold\"]",
            labels: &[
                (
                    "player.inventory[\"gold\"]",
                    "host index capability is not readable",
                ),
                (
                    "player.inventory",
                    "enable readable host index access for this type",
                ),
            ],
        },
        DiagnosticCase {
            source: "fn main(player: Player) { player.inventory[\"gold\"] = 1; return 0; }",
            capability: Some(index_capability(
                false,
                false,
                false,
                false,
                TypeFact::STRING,
            )),
            code: "analysis::host_index_not_writable",
            message: "type `Inventory` does not allow host index writes",
            primary: "player.inventory[\"gold\"] = 1",
            labels: &[
                (
                    "player.inventory[\"gold\"] = 1",
                    "host index capability is not writable",
                ),
                (
                    "player.inventory",
                    "enable writable host index access for this type",
                ),
            ],
        },
        DiagnosticCase {
            source: "fn main(player: Player) { player.inventory[\"gold\"] += 1; return 0; }",
            capability: Some(index_capability(
                false,
                false,
                false,
                false,
                TypeFact::STRING,
            )),
            code: "analysis::host_index_not_mutable",
            message: "type `Inventory` does not allow host index mutations",
            primary: "player.inventory[\"gold\"] += 1",
            labels: &[
                (
                    "player.inventory[\"gold\"] += 1",
                    "host index capability is not addable",
                ),
                (
                    "player.inventory",
                    "enable addable host index access for this type",
                ),
            ],
        },
        DiagnosticCase {
            source: "fn main(player: Player) { player.inventory[\"gold\"].remove(); return 0; }",
            capability: Some(index_capability(
                false,
                false,
                false,
                false,
                TypeFact::STRING,
            )),
            code: "analysis::host_index_not_removable",
            message: "type `Inventory` does not allow host index removals",
            primary: "player.inventory[\"gold\"].remove()",
            labels: &[
                (
                    "player.inventory[\"gold\"].remove()",
                    "host index capability is not removable",
                ),
                (
                    "player.inventory",
                    "enable removable host index access for this type",
                ),
            ],
        },
        DiagnosticCase {
            source: "fn main(player: Player) { return player.inventory[\"gold\"]; }",
            capability: Some(index_capability(true, false, false, false, TypeFact::I64)),
            code: "analysis::host_index_key_mismatch",
            message: "host index key for `Inventory` must be `i64`",
            primary: "player.inventory[\"gold\"]",
            labels: &[("\"gold\"", "index expression has type `String`")],
        },
        DiagnosticCase {
            source: "fn main(player: Player) { player.level = 2; return 0; }",
            capability: Some(index_capability(true, true, true, true, TypeFact::STRING)),
            code: "analysis::field_not_writable",
            message: "field is read-only for script writes",
            primary: "player.level = 2",
            labels: &[
                ("player.level = 2", "assignment targets a read-only field"),
                (
                    "player.level = 2",
                    "write through an exposed method or a writable field instead",
                ),
            ],
        },
    ];

    for (offset, case) in cases.into_iter().enumerate() {
        let source = SourceId::new(800 + u32::try_from(offset).expect("source offset"));
        let (graph, main) = graph(source, case.source);
        let schema = host_schema(case.capability, false, true);
        let function = FunctionId::new(80_000 + u128::try_from(offset).expect("function offset"));
        let generation = generation(&graph, &schema, main, function);
        let view = generation.view(function).expect("main analysis");
        let [diagnostic] = view.validation_diagnostics() else {
            panic!("one host diagnostic for {}", case.code);
        };
        assert_eq!(diagnostic.code.as_deref(), Some(case.code));
        assert_eq!(diagnostic.message, case.message);
        assert_eq!(
            span_text(case.source, diagnostic.span.expect("diagnostic span")),
            case.primary
        );
        assert_eq!(diagnostic.labels.len(), case.labels.len());
        for (label, (expected_span, expected_message)) in diagnostic.labels.iter().zip(case.labels)
        {
            assert_eq!(span_text(case.source, label.span), *expected_span);
            assert_eq!(label.message, *expected_message);
        }

        if case.code == "analysis::host_index_not_supported" {
            let expression = expression_exact(&graph, source, case.source, case.primary, 0);
            let fact = view
                .host_access_use(expression)
                .expect("unsupported index still has a use fact");
            assert_eq!(fact.kind, HostAccessUseKind::Read);
            assert_eq!(fact.accessed_index, Some(0));
            assert!(matches!(
                fact.indexes[0].capability,
                HostIndexCapabilityResolutionFact::Missing
            ));
        }
    }
}

#[test]
fn host_access_facts_keep_path_indexes_effects_and_independent_uses() {
    let source = SourceId::new(808);
    let text = r#"
fn main(player: Player, other: Player) {
    let before = player.inventory[other.level].amount;
    player.inventory[other.level].amount += other.level;
    player.inventory[other.level].remove();
    player.inventory.push(other.level);
    player.save(other.level);
    return before;
}
"#;
    let (graph, main) = graph(source, text);
    let schema = host_schema(
        Some(index_capability(true, true, true, true, TypeFact::I64)),
        true,
        true,
    );
    let function = FunctionId::new(80_808);
    let generation = generation(&graph, &schema, main, function);
    let view = generation.view(function).expect("main analysis");
    assert_eq!(view.validation_diagnostics(), &[]);

    let read = expression_exact(
        &graph,
        source,
        text,
        "player.inventory[other.level].amount",
        0,
    );
    let read_fact = view.host_access_use(read).expect("host field read");
    assert_eq!(read_fact.kind, HostAccessUseKind::Read);
    assert_eq!(read_fact.indexes.len(), 1);
    assert_eq!(read_fact.accessed_index, None);
    assert_eq!(view.effect(read), Some(&RegistryEffectFact::host_read()));

    let assignment = expression_exact(
        &graph,
        source,
        text,
        "player.inventory[other.level].amount += other.level",
        0,
    );
    let assignment_fact = view
        .host_access_use(assignment)
        .expect("host field mutation");
    assert_eq!(assignment_fact.kind, HostAccessUseKind::Mutate);
    assert_eq!(assignment_fact.indexes.len(), 1);
    assert_eq!(assignment_fact.accessed_index, None);
    assert_eq!(
        view.effect(assignment),
        Some(&RegistryEffectFact::host_write())
    );

    let remove = expression_exact(
        &graph,
        source,
        text,
        "player.inventory[other.level].remove()",
        0,
    );
    let remove_fact = view.host_access_use(remove).expect("host remove");
    assert_eq!(remove_fact.kind, HostAccessUseKind::Remove);
    assert_eq!(remove_fact.accessed_index, Some(0));

    let push = expression_exact(
        &graph,
        source,
        text,
        "player.inventory.push(other.level)",
        0,
    );
    assert_eq!(
        view.host_access_use(push).map(|fact| fact.kind),
        Some(HostAccessUseKind::Push)
    );

    let call = expression_exact(&graph, source, text, "player.save(other.level)", 0);
    let call_fact = view.host_access_use(call).expect("host method call");
    assert_eq!(call_fact.kind, HostAccessUseKind::Call);
    assert_eq!(call_fact.effect, RegistryEffectFact::host_write());

    assert_eq!(text.match_indices("other.level").count(), 6);
    for occurrence in 0..6 {
        let independent = expression_exact(&graph, source, text, "other.level", occurrence);
        assert_eq!(
            view.host_access_use(independent).map(|fact| fact.kind),
            Some(HostAccessUseKind::Read)
        );
        assert_eq!(
            view.effect(independent),
            Some(&RegistryEffectFact::host_read())
        );
    }

    let read_index = expression_exact(&graph, source, text, "player.inventory[other.level]", 0);
    assert!(view.host_access_use(read_index).is_none());
}

#[test]
fn host_method_uses_retain_root_and_parenthesized_host_targets() {
    let source = SourceId::new(809);
    let text = r#"
fn main(player: Player) {
    player.save();
    (player).save();
}
"#;
    let (graph, main) = graph(source, text);
    let schema = host_schema(None, true, true);
    let function = FunctionId::new(80_809);
    let generation = generation(&graph, &schema, main, function);
    let view = generation.view(function).expect("main analysis");
    let body = graph.function_body(main).expect("main body");
    let expected_root =
        RegistryTypeTargetFact::new("Player", TypeId::new(801), Some(HostTypeId::new(801)));

    let root_call = expression_exact(&graph, source, text, "player.save()", 0);
    let root_use = view.host_access_use(root_call).expect("root host call use");
    let root_target = body
        .expression(root_use.target)
        .expect("root host call target");
    assert_eq!(span_text(text, root_target.origin.span), "player");
    let root_path = view
        .host_path_target(root_use.target)
        .expect("root host target fact");
    assert_eq!(root_path.root, root_use.target);
    assert_eq!(root_path.root_type, expected_root);
    assert!(root_path.segments.is_empty());

    let parenthesized_call = expression_exact(&graph, source, text, "(player).save()", 0);
    let parenthesized_use = view
        .host_access_use(parenthesized_call)
        .expect("parenthesized host call use");
    let parenthesized_target = body
        .expression(parenthesized_use.target)
        .expect("parenthesized host call target");
    assert_eq!(
        span_text(text, parenthesized_target.origin.span),
        "(player)"
    );
    let parenthesized_path = view
        .host_path_target(parenthesized_use.target)
        .expect("parenthesized host target fact");
    assert_eq!(
        span_text(
            text,
            body.expression(parenthesized_path.root)
                .expect("parenthesized host root")
                .origin
                .span,
        ),
        "player"
    );
    assert_eq!(parenthesized_path.root_type, expected_root);
    assert!(parenthesized_path.segments.is_empty());
}

#[test]
fn root_host_index_without_capability_retains_its_stable_owner() {
    let source = SourceId::new(810);
    let text = "fn main(player: Player) { return player[\"gold\"]; }";
    let (graph, main) = graph(source, text);
    let schema = host_schema(None, false, true);
    let function = FunctionId::new(80_810);
    let generation = generation(&graph, &schema, main, function);
    let view = generation.view(function).expect("main analysis");
    let expression = expression_exact(&graph, source, text, "player[\"gold\"]", 0);
    let fact = view
        .host_access_use(expression)
        .expect("root host index use");

    assert_eq!(fact.accessed_index, Some(0));
    assert_eq!(fact.indexes[0].owner.name, "Player");
    assert!(matches!(
        fact.indexes[0].capability,
        HostIndexCapabilityResolutionFact::Missing
    ));
    assert_eq!(
        view.validation_diagnostics()[0].code.as_deref(),
        Some("analysis::host_index_not_supported")
    );
}

#[test]
fn shared_host_body_access_facts_are_executable_qualified() {
    let source = SourceId::new(811);
    let text = r#"
trait Touch {
    fn touch(self) { self.level += 1; }
}
"#;
    let (graph, _) = graph(source, text);
    let body = graph
        .bodies()
        .find(|body| matches!(body.owner, HirBodyOwner::TraitDefaultMethod(_)))
        .expect("trait default body");
    let assignment = expression_exact(&graph, source, text, "self.level += 1", 0);
    let mut schema = RegistryFacts::default();
    for (offset, name, writable) in [(0_u128, "Writable", true), (1, "ReadOnly", false)] {
        let target = RegistryTypeTargetFact::new(
            name,
            TypeId::new(820 + offset),
            Some(HostTypeId::new(
                820 + u64::try_from(offset).expect("host offset"),
            )),
        );
        schema.insert_type(name, TypeFact::host(name));
        schema.insert_type_target(target.clone());
        insert_field(
            &mut schema,
            &target,
            "level",
            FieldId::new(830 + offset),
            TypeFact::I64,
            writable,
        );
    }
    let writable_function = FunctionId::new(80_811);
    let read_only_function = FunctionId::new(80_812);
    let generation = ExecutableAnalysisGeneration::from_module_graph_and_schema(
        &graph,
        &schema,
        [
            ExecutableAnalysisInput::new(writable_function, body.id)
                .with_receiver(ExecutableReceiverInput::new(TypeFact::host("Writable"))),
            ExecutableAnalysisInput::new(read_only_function, body.id)
                .with_receiver(ExecutableReceiverInput::new(TypeFact::host("ReadOnly"))),
        ],
    )
    .expect("qualified host access analysis");
    let writable = generation
        .view(writable_function)
        .expect("writable analysis");
    let read_only = generation
        .view(read_only_function)
        .expect("read-only analysis");

    assert_eq!(
        writable.host_access_use(assignment).map(|fact| fact.kind),
        Some(HostAccessUseKind::Mutate)
    );
    assert_eq!(
        read_only.host_access_use(assignment).map(|fact| fact.kind),
        Some(HostAccessUseKind::Mutate)
    );
    assert_eq!(writable.validation_diagnostics(), &[]);
    assert_eq!(
        read_only.validation_diagnostics()[0].code.as_deref(),
        Some("analysis::field_not_writable")
    );
}

#[test]
fn traversal_indexes_retain_denied_metadata_without_becoming_direct_accesses() {
    let source = SourceId::new(809);
    let source_text = r#"
fn main(player: Player) {
    player.inventory["gold"].amount += 1;
    player.inventory["gold"].save();
    return player.inventory["gold"].amount;
}
"#;
    let (graph, main) = graph(source, source_text);
    let schema = host_schema(
        Some(index_capability(
            false,
            false,
            false,
            false,
            TypeFact::STRING,
        )),
        true,
        true,
    );
    let function = FunctionId::new(80_809);
    let generation = generation(&graph, &schema, main, function);
    let view = generation.view(function).expect("main analysis");
    assert_eq!(view.validation_diagnostics(), &[]);

    for (expression_text, occurrence, kind) in [
        (
            "player.inventory[\"gold\"].amount += 1",
            0,
            HostAccessUseKind::Mutate,
        ),
        (
            "player.inventory[\"gold\"].save()",
            0,
            HostAccessUseKind::Call,
        ),
        (
            "player.inventory[\"gold\"].amount",
            1,
            HostAccessUseKind::Read,
        ),
    ] {
        let expression = expression_exact(&graph, source, source_text, expression_text, occurrence);
        let fact = view.host_access_use(expression).expect("host path use");
        assert_eq!(fact.kind, kind);
        assert_eq!(fact.indexes.len(), 1);
        assert_eq!(fact.accessed_index, None);
        assert!(matches!(
            &fact.indexes[0].capability,
            HostIndexCapabilityResolutionFact::Registered(capability)
                if !capability.readable
                    && !capability.writable
                    && !capability.addable
                    && !capability.removable
        ));
    }
}

struct DiagnosticCase {
    source: &'static str,
    capability: Option<RegistryIndexCapabilityFact>,
    code: &'static str,
    message: &'static str,
    primary: &'static str,
    labels: &'static [(&'static str, &'static str)],
}

fn index_capability(
    readable: bool,
    writable: bool,
    addable: bool,
    removable: bool,
    key: TypeFact,
) -> RegistryIndexCapabilityFact {
    RegistryIndexCapabilityFact {
        owner: "Inventory".to_owned(),
        readable,
        writable,
        addable,
        removable,
        key,
        value: TypeFact::host("Entry"),
    }
}

fn host_schema(
    capability: Option<RegistryIndexCapabilityFact>,
    level_writable: bool,
    amount_writable: bool,
) -> RegistryFacts {
    let player =
        RegistryTypeTargetFact::new("Player", TypeId::new(801), Some(HostTypeId::new(801)));
    let inventory =
        RegistryTypeTargetFact::new("Inventory", TypeId::new(802), Some(HostTypeId::new(802)));
    let entry = RegistryTypeTargetFact::new("Entry", TypeId::new(803), Some(HostTypeId::new(803)));
    let mut schema = RegistryFacts::default();
    for (name, fact, target) in [
        ("Player", TypeFact::host("Player"), player.clone()),
        ("Inventory", TypeFact::host("Inventory"), inventory.clone()),
        ("Entry", TypeFact::host("Entry"), entry.clone()),
    ] {
        schema.insert_type(name, fact);
        schema.insert_type_target(target);
    }
    insert_field(
        &mut schema,
        &player,
        "inventory",
        FieldId::new(811),
        TypeFact::host("Inventory"),
        true,
    );
    insert_field(
        &mut schema,
        &player,
        "level",
        FieldId::new(812),
        TypeFact::I64,
        level_writable,
    );
    insert_field(
        &mut schema,
        &entry,
        "amount",
        FieldId::new(813),
        TypeFact::I64,
        amount_writable,
    );
    if let Some(capability) = capability {
        schema.insert_index_capability(capability);
    }
    for owner in ["Player", "Entry"] {
        schema.insert_method(
            owner,
            "save",
            TypeFact::function(Vec::new(), TypeFact::UNIT),
        );
        schema.insert_method_signature(
            owner,
            "save",
            CallableSignatureFact::new(std::iter::empty::<CallableParameterFact>(), TypeFact::UNIT),
        );
        schema.insert_method_effect(owner, "save", RegistryEffectFact::host_write());
    }
    schema
}

fn insert_field(
    schema: &mut RegistryFacts,
    owner: &RegistryTypeTargetFact,
    name: &str,
    field: FieldId,
    fact: TypeFact,
    writable: bool,
) {
    schema.insert_field(&owner.name, name, fact);
    let access = RegistryFieldAccessFact {
        owner: owner.name.clone(),
        name: name.to_owned(),
        readable: true,
        writable,
        reflect_readable: false,
        reflect_writable: false,
        required_permissions: Vec::new(),
    };
    schema.insert_field_access(access.clone());
    schema.insert_field_target(RegistryFieldTargetFact::new(
        owner.semantic,
        &owner.name,
        name,
        field,
        Some(field),
        false,
        access,
    ));
}

fn graph(source: SourceId, text: &str) -> (ModuleGraph, HirDeclId) {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game"),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let main = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .map(|declaration| declaration.id)
        .unwrap_or(HirDeclId::new(u32::MAX));
    (graph, main)
}

fn generation(
    graph: &ModuleGraph,
    schema: &RegistryFacts,
    declaration: HirDeclId,
    function: FunctionId,
) -> ExecutableAnalysisGeneration {
    let body = graph.function_body(declaration).expect("main body");
    ExecutableAnalysisGeneration::from_module_graph_and_schema(
        graph,
        schema,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("executable host analysis")
}

fn expression_exact(
    graph: &ModuleGraph,
    source: SourceId,
    text: &str,
    expression: &str,
    occurrence: usize,
) -> HirExprId {
    let start = text
        .match_indices(expression)
        .nth(occurrence)
        .map(|(start, _)| start)
        .unwrap_or_else(|| panic!("occurrence {occurrence} of {expression:?}"));
    graph
        .expression_at_span(Span::new(
            source,
            u32::try_from(start).expect("expression start"),
            u32::try_from(start + expression.len()).expect("expression end"),
        ))
        .expect("HIR expression at exact span")
}

fn span_text(text: &str, span: Span) -> &str {
    &text[usize::try_from(span.start).expect("span start")
        ..usize::try_from(span.end).expect("span end")]
}
