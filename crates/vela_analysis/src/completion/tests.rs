use vela_common::{HostMethodId, SourceId};
use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId};
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_reflect::modules::FunctionDesc;
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
    TypeKind, TypeRegistry, VariantDesc,
};

use super::*;

#[test]
fn receiver_completions_include_fields_and_methods_for_host_or_record_facts() {
    let facts = registry_facts();

    let completions = member_completions(&facts, &TypeFact::host("Player"));

    assert!(completions.contains(&CompletionItem::new(
        "level",
        CompletionKind::Field,
        TypeFact::I64,
    )));
    assert!(completions.contains(&CompletionItem::new(
        "grant_exp",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
    )));
}

#[test]
fn enum_completions_include_variants_and_variant_fields() {
    let facts = registry_facts();

    let variants = member_completions(&facts, &TypeFact::enum_type("QuestState", None::<String>));
    assert_eq!(
        variants,
        vec![CompletionItem::new(
            "Active",
            CompletionKind::Variant,
            TypeFact::enum_type("QuestState", Some("Active")),
        )]
    );

    let fields = member_completions(&facts, &TypeFact::enum_type("QuestState", Some("Active")));
    assert_eq!(
        fields,
        vec![CompletionItem::new(
            "quest_id",
            CompletionKind::Field,
            TypeFact::STRING,
        )]
    );
}

#[test]
fn global_completions_include_types_traits_and_functions() {
    let facts = registry_facts();
    let completions = global_completions(&facts);

    assert!(completions.contains(&CompletionItem::new(
        "Player",
        CompletionKind::Type,
        TypeFact::host("Player"),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "Damageable",
        CompletionKind::Trait,
        TypeFact::trait_type("Damageable"),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "game::reward::grant",
        CompletionKind::Function,
        TypeFact::function(vec![TypeFact::host("Player")], TypeFact::BOOL),
    )));
}

#[test]
fn trait_receiver_completions_include_trait_methods() {
    let facts = registry_facts();

    let completions = member_completions(&facts, &TypeFact::trait_type("Damageable"));

    assert_eq!(
        completions,
        vec![CompletionItem::new(
            "damage",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL),
        )]
    );
}

#[test]
fn receiver_completions_include_stdlib_collection_and_string_methods() {
    let facts = registry_facts();

    let map = member_completions(&facts, &TypeFact::map(TypeFact::STRING, TypeFact::I64));
    assert!(map.contains(&CompletionItem::new(
        "get",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::option(TypeFact::I64)),
    )));
    assert!(map.contains(&CompletionItem::new(
        "filter",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(
                vec![TypeFact::STRING, TypeFact::I64],
                TypeFact::BOOL,
            )],
            TypeFact::map(TypeFact::STRING, TypeFact::I64),
        ),
    )));
    assert!(map.contains(&CompletionItem::new(
        "merge",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::map(TypeFact::STRING, TypeFact::I64)],
            TypeFact::map(TypeFact::STRING, TypeFact::I64),
        ),
    )));
    assert!(map.contains(&CompletionItem::new(
        "find",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(
                vec![TypeFact::STRING, TypeFact::I64],
                TypeFact::BOOL,
            )],
            TypeFact::option(TypeFact::record("MapEntry")),
        ),
    )));
    assert!(map.contains(&CompletionItem::new(
        "any",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(
                vec![TypeFact::STRING, TypeFact::I64],
                TypeFact::BOOL,
            )],
            TypeFact::BOOL,
        ),
    )));
    assert!(map.contains(&CompletionItem::new(
        "count",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(
                vec![TypeFact::STRING, TypeFact::I64],
                TypeFact::BOOL,
            )],
            TypeFact::I64,
        ),
    )));
    let array = member_completions(&facts, &TypeFact::array(TypeFact::STRING));
    assert!(array.contains(&CompletionItem::new(
        "first",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::STRING)),
    )));
    assert!(array.contains(&CompletionItem::new(
        "last",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::STRING)),
    )));
    assert!(array.contains(&CompletionItem::new(
        "join",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::STRING),
    )));
    assert!(array.contains(&CompletionItem::new(
        "contains",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL),
    )));
    assert!(array.contains(&CompletionItem::new(
        "distinct",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::array(TypeFact::STRING)),
    )));
    assert!(array.contains(&CompletionItem::new(
        "reverse",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::array(TypeFact::STRING)),
    )));
    assert!(array.contains(&CompletionItem::new(
        "slice",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::I64, TypeFact::I64],
            TypeFact::array(TypeFact::STRING),
        ),
    )));

    let set = member_completions(&facts, &TypeFact::set(TypeFact::STRING));
    assert!(set.contains(&CompletionItem::new(
        "map",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::Any)],
            TypeFact::set(TypeFact::Any),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "filter",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)],
            TypeFact::set(TypeFact::STRING),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "find",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)],
            TypeFact::option(TypeFact::STRING),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "any",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)],
            TypeFact::BOOL,
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "all",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)],
            TypeFact::BOOL,
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "count",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::function(vec![TypeFact::STRING], TypeFact::BOOL)],
            TypeFact::I64,
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "union",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::set(TypeFact::STRING)],
            TypeFact::set(TypeFact::STRING),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "intersection",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::set(TypeFact::STRING)],
            TypeFact::set(TypeFact::STRING),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "difference",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::set(TypeFact::STRING)],
            TypeFact::set(TypeFact::STRING),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "symmetric_difference",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::set(TypeFact::STRING)],
            TypeFact::set(TypeFact::STRING),
        ),
    )));
    assert!(set.contains(&CompletionItem::new(
        "is_subset",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::set(TypeFact::STRING)], TypeFact::BOOL),
    )));
    assert!(set.contains(&CompletionItem::new(
        "is_superset",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::set(TypeFact::STRING)], TypeFact::BOOL),
    )));
    assert!(set.contains(&CompletionItem::new(
        "is_disjoint",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::set(TypeFact::STRING)], TypeFact::BOOL),
    )));

    let string = member_completions(&facts, &TypeFact::STRING);
    assert!(string.contains(&CompletionItem::new(
        "find",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::option(TypeFact::I64)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "strip_prefix",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::option(TypeFact::STRING)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "strip_suffix",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::option(TypeFact::STRING)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "replace",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING, TypeFact::STRING], TypeFact::STRING),
    )));
    assert!(string.contains(&CompletionItem::new(
        "repeat",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::I64], TypeFact::STRING),
    )));
    assert!(string.contains(&CompletionItem::new(
        "trim_start",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::STRING),
    )));
    assert!(string.contains(&CompletionItem::new(
        "trim_end",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::STRING),
    )));
    assert!(string.contains(&CompletionItem::new(
        "slice",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::STRING),
    )));
    assert!(string.contains(&CompletionItem::new(
        "split",
        CompletionKind::Method,
        TypeFact::function(vec![TypeFact::STRING], TypeFact::array(TypeFact::STRING)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "split_once",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::STRING],
            TypeFact::option(TypeFact::array(TypeFact::STRING))
        ),
    )));
    assert!(string.contains(&CompletionItem::new(
        "split_lines",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::array(TypeFact::STRING)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "split_whitespace",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::array(TypeFact::STRING)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_i8",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I8)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_i16",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I16)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_i32",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I32)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_i64",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I64)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_u8",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::U8)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_u16",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::U16)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_u32",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::U32)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_u64",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::U64)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_f32",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::F32)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_f64",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::F64)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_bool",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::BOOL)),
    )));
    assert!(string.contains(&CompletionItem::new(
        "parse_char",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::CHAR)),
    )));

    let option = member_completions(&facts, &TypeFact::option(TypeFact::I64));
    assert!(option.contains(&CompletionItem::new(
        "unwrap_or",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::Any],
            TypeFact::union([TypeFact::I64, TypeFact::Any]),
        ),
    )));
    assert!(option.contains(&CompletionItem::new(
        "ok_or",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::Any],
            TypeFact::result(TypeFact::I64, TypeFact::Any),
        ),
    )));
    let nested_option =
        member_completions(&facts, &TypeFact::option(TypeFact::option(TypeFact::I64)));
    assert!(nested_option.contains(&CompletionItem::new(
        "flatten",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I64)),
    )));

    let result = member_completions(&facts, &TypeFact::result(TypeFact::I64, TypeFact::STRING));
    assert!(result.contains(&CompletionItem::new(
        "unwrap_or",
        CompletionKind::Method,
        TypeFact::function(
            vec![TypeFact::Any],
            TypeFact::union([TypeFact::I64, TypeFact::Any]),
        ),
    )));
    assert!(result.contains(&CompletionItem::new(
        "to_option",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::I64)),
    )));
    assert!(result.contains(&CompletionItem::new(
        "to_error_option",
        CompletionKind::Method,
        TypeFact::function(Vec::new(), TypeFact::option(TypeFact::STRING)),
    )));
    let nested_result = member_completions(
        &facts,
        &TypeFact::result(
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
            TypeFact::record("OuterError"),
        ),
    );
    assert!(nested_result.contains(&CompletionItem::new(
        "flatten",
        CompletionKind::Method,
        TypeFact::function(
            Vec::new(),
            TypeFact::result(
                TypeFact::I64,
                TypeFact::union([TypeFact::record("OuterError"), TypeFact::STRING]),
            ),
        ),
    )));
}

#[test]
fn global_completions_include_stdlib_functions() {
    let facts = registry_facts();
    let completions = global_completions(&facts);
    let number = TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]);

    assert!(completions.contains(&CompletionItem::new(
        "option::unwrap_or",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::option(TypeFact::Any), TypeFact::Any],
            TypeFact::Any
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "option::ok_or",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::option(TypeFact::Any), TypeFact::Any],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "option::flatten",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::option(TypeFact::option(TypeFact::Any))],
            TypeFact::option(TypeFact::Any),
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "result::to_option",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::option(TypeFact::Any),
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "result::to_error_option",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::result(TypeFact::Any, TypeFact::Any)],
            TypeFact::option(TypeFact::Any),
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "result::flatten",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::result(
                TypeFact::result(TypeFact::Any, TypeFact::Any),
                TypeFact::Any,
            )],
            TypeFact::result(TypeFact::Any, TypeFact::Any),
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "math::random",
        CompletionKind::Function,
        TypeFact::function(vec![TypeFact::I64, TypeFact::I64], TypeFact::I64),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "time::elapsed_since",
        CompletionKind::Function,
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "math::lerp",
        CompletionKind::Function,
        TypeFact::function(vec![number.clone(), number.clone(), number], TypeFact::F64),
    )));
    let number = TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]);
    assert!(completions.contains(&CompletionItem::new(
        "math::move_towards",
        CompletionKind::Function,
        TypeFact::function(vec![number.clone(), number.clone(), number.clone()], number),
    )));
    let number = TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]);
    assert!(completions.contains(&CompletionItem::new(
        "math::distance2d",
        CompletionKind::Function,
        TypeFact::function(
            vec![number.clone(), number.clone(), number.clone(), number],
            TypeFact::F64
        ),
    )));
    let number = TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]);
    assert!(completions.contains(&CompletionItem::new(
        "math::distance3d",
        CompletionKind::Function,
        TypeFact::function(
            vec![
                number.clone(),
                number.clone(),
                number.clone(),
                number.clone(),
                number.clone(),
                number,
            ],
            TypeFact::F64
        ),
    )));
    let number = TypeFact::Union(vec![TypeFact::I64, TypeFact::F64]);
    assert!(completions.contains(&CompletionItem::new(
        "math::pow",
        CompletionKind::Function,
        TypeFact::function(vec![number.clone(), number.clone()], number),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "math::sqrt",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])],
            TypeFact::F64
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "math::sign",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])],
            TypeFact::I64
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "math::round",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::Union(vec![TypeFact::I64, TypeFact::F64])],
            TypeFact::I64
        ),
    )));
}

#[test]
fn local_completions_include_function_scope_bindings() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game"),
        r#"
            struct Player { level: i64 }
            fn grant(player: Player, amount: i64) -> bool {
                let rewards: map = {};
                let inferred = 10;
                for reward in [] {
                    let amount: string = reward;
                }
                return amount > 0;
            }
            "#,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "grant")
        .expect("grant declaration")
        .id;
    let facts = AnalysisFacts::from_module_graph(&graph);

    let completions = local_completions(&graph, &facts, declaration);

    assert!(completions.contains(&CompletionItem::new(
        "player",
        CompletionKind::Binding,
        TypeFact::record("game::Player"),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "rewards",
        CompletionKind::Binding,
        TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "inferred",
        CompletionKind::Binding,
        TypeFact::Unknown,
    )));
    assert!(completions.contains(&CompletionItem::new(
        "amount",
        CompletionKind::Binding,
        TypeFact::I64,
    )));
    assert!(completions.contains(&CompletionItem::new(
        "amount",
        CompletionKind::Binding,
        TypeFact::STRING,
    )));
}

#[test]
fn declaration_completions_include_script_declarations() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game::player"),
        r#"
            pub struct Player { level: i64 }
            pub enum QuestState { Active { quest_id: string }, Done }
            pub trait Damageable {
                fn damage(self, amount: i64) -> bool;
            }
            pub const START_LEVEL: i64 = 1
            pub fn grant(player: Player, amount: i64) -> bool {
                return amount > 0;
            }
            impl Damageable for Player {
                fn damage(self, amount: i64) -> bool {
                    return amount > 0;
                }
            }
            "#,
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(2),
        ModulePath::from_qualified("game::reward"),
        r#"
            pub fn grant(amount: i64) -> i64 {
                return amount + 1;
            }
            "#,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let facts = AnalysisFacts::from_module_graph(&graph);

    let completions = declaration_completions(&graph, &facts);

    assert!(completions.contains(&CompletionItem::new(
        "game::player::Player",
        CompletionKind::Type,
        TypeFact::record("game::player::Player"),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "game::player::QuestState",
        CompletionKind::Type,
        TypeFact::enum_type("game::player::QuestState", None::<String>),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "game::player::Damageable",
        CompletionKind::Trait,
        TypeFact::trait_type("game::player::Damageable"),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "game::player::START_LEVEL",
        CompletionKind::Const,
        TypeFact::I64,
    )));
    assert!(completions.contains(&CompletionItem::new(
        "game::player::grant",
        CompletionKind::Function,
        TypeFact::function(
            vec![TypeFact::record("game::player::Player"), TypeFact::I64],
            TypeFact::BOOL,
        ),
    )));
    assert!(completions.contains(&CompletionItem::new(
        "game::reward::grant",
        CompletionKind::Function,
        TypeFact::function(vec![TypeFact::I64], TypeFact::I64),
    )));
    assert!(
        completions
            .iter()
            .all(|completion| completion.label != "game::player::Damageable::Player")
    );
}

#[test]
fn module_completions_include_module_paths_and_prefixes() {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game::player"),
        "pub fn level() { return 1; }",
    ));
    graph.add_source(ModuleSource::new(
        SourceId::new(2),
        ModulePath::from_qualified("game::reward"),
        "pub fn grant() { return 2; }",
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let completions = module_completions(&graph);

    assert_eq!(
        completions,
        vec![
            CompletionItem::new("game", CompletionKind::Module, TypeFact::module("game")),
            CompletionItem::new(
                "game::player",
                CompletionKind::Module,
                TypeFact::module("game::player"),
            ),
            CompletionItem::new(
                "game::reward",
                CompletionKind::Module,
                TypeFact::module("game::reward"),
            ),
        ]
    );
}

fn registry_facts() -> RegistryFacts {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("i64"))
            .method(
                MethodDesc::new(HostMethodId::new(1), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("i64"))
                    .return_type("bool"),
            ),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestState"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(2), "quest_id").type_hint("string")),
            ),
    );
    registry.register_trait(
        TraitDesc::new("Damageable").method(
            TraitMethodDesc::new(MethodId::new(1), "damage")
                .param(MethodParamDesc::new("amount").type_hint("i64"))
                .return_type("bool"),
        ),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game::reward::grant")
            .param(vela_reflect::modules::FunctionParamDesc::new("player").type_hint("Player"))
            .return_type("bool"),
    );
    RegistryFacts::from_registry(&registry)
}
