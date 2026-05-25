use vela_common::{FieldId, MethodId, Span, TypeId, VariantId};
use vela_hir::{Declaration, DeclarationKind, EnumVariantFieldsHint, ModuleGraph};

use crate::{
    FieldDesc, MethodParamDesc, SchemaHash, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
    TypeKind, TypeRegistry, VariantDesc, script_attrs::ReflectedScriptAttrs,
};

impl TypeRegistry {
    pub fn register_script_types(&mut self, graph: &ModuleGraph) {
        for declaration in graph.declarations() {
            match declaration.kind {
                DeclarationKind::Struct => {
                    let Some(shape) = graph.struct_shape(declaration.id) else {
                        continue;
                    };
                    let type_name = qualified_type_name(graph, declaration);
                    let mut desc = shape.fields.iter().fold(
                        TypeDesc::new(TypeKey::new(stable_type_id(&type_name), type_name.clone()))
                            .kind(TypeKind::ScriptStruct)
                            .schema_hash(struct_schema_hash(&type_name, shape))
                            .source_span(declaration.span),
                        |desc, field| {
                            desc.field(apply_field_attrs(
                                apply_field_type_hint(
                                    FieldDesc::new(
                                        stable_field_id(&type_name, &field.name),
                                        field.name.clone(),
                                    )
                                    .defaulted(field.default_value_span.is_some())
                                    .source_span(field.span),
                                    &field.type_hint,
                                ),
                                &field.attrs,
                            ))
                        },
                    );
                    apply_type_attrs(&mut desc, graph.declaration_attrs(declaration.id));
                    self.register(desc);
                }
                DeclarationKind::Enum => {
                    let Some(shape) = graph.enum_shape(declaration.id) else {
                        continue;
                    };
                    let type_name = qualified_type_name(graph, declaration);
                    let mut desc = shape.variants.iter().fold(
                        TypeDesc::new(TypeKey::new(stable_type_id(&type_name), type_name.clone()))
                            .kind(TypeKind::ScriptEnum)
                            .schema_hash(enum_schema_hash(&type_name, shape))
                            .source_span(declaration.span),
                        |desc, variant| {
                            let variant_owner = enum_variant_owner(&type_name, &variant.name);
                            let variant_desc =
                                enum_variant_fields(&variant.fields).into_iter().fold(
                                    apply_variant_attrs(
                                        VariantDesc::new(
                                            stable_variant_id(&type_name, &variant.name),
                                            variant.name.clone(),
                                        )
                                        .source_span(variant.span),
                                        &variant.attrs,
                                    ),
                                    |desc, field| {
                                        desc.field(apply_field_attrs(
                                            apply_field_type_hint_display(
                                                FieldDesc::new(
                                                    stable_field_id(&variant_owner, &field.name),
                                                    field.name,
                                                )
                                                .defaulted(field.has_default)
                                                .source_span(field.span),
                                                &field.type_hint,
                                            ),
                                            &field.attrs,
                                        ))
                                    },
                                );
                            desc.variant(variant_desc)
                        },
                    );
                    apply_type_attrs(&mut desc, graph.declaration_attrs(declaration.id));
                    self.register(desc);
                }
                DeclarationKind::Trait => {
                    let Some(shape) = graph.trait_shape(declaration.id) else {
                        continue;
                    };
                    let trait_name = qualified_type_name(graph, declaration);
                    let mut desc = shape.methods.iter().fold(
                        TraitDesc::new(trait_name.clone()).source_span(declaration.span),
                        |desc, method| {
                            desc.method(apply_trait_method_attrs(
                                TraitMethodDesc::new(
                                    stable_trait_method_id(&trait_name, &method.name),
                                    method.name.clone(),
                                )
                                .defaulted(method.has_default)
                                .source_span(method.span),
                                &method.signature,
                                &method.attrs,
                            ))
                        },
                    );
                    apply_trait_attrs(&mut desc, graph.declaration_attrs(declaration.id));
                    self.register_trait(desc);
                }
                DeclarationKind::Const | DeclarationKind::Function | DeclarationKind::Impl => {}
            }
        }

        for declaration in graph.declarations() {
            if declaration.kind != DeclarationKind::Impl {
                continue;
            }
            let Some(metadata) = graph.impl_metadata(declaration.id) else {
                continue;
            };
            let trait_name = qualified_path_name(graph, declaration, &metadata.trait_path);
            let target_name = qualified_path_name(graph, declaration, &metadata.target_path);
            let trait_desc = self
                .trait_by_name(&trait_name)
                .cloned()
                .unwrap_or_else(|| TraitDesc::new(trait_name));
            if let Some(target) = self.type_by_name_mut(&target_name) {
                target.traits.push(trait_desc);
            }
        }
    }
}

fn apply_type_attrs(desc: &mut TypeDesc, attrs: &[vela_hir::HirAttribute]) {
    let reflected = ReflectedScriptAttrs::from_hir(attrs);
    desc.attrs = reflected.attrs;
    desc.docs = reflected.docs;
}

fn apply_trait_attrs(desc: &mut TraitDesc, attrs: &[vela_hir::HirAttribute]) {
    let reflected = ReflectedScriptAttrs::from_hir(attrs);
    desc.attrs = reflected.attrs;
    desc.docs = reflected.docs;
}

fn apply_field_attrs(mut desc: FieldDesc, attrs: &[vela_hir::HirAttribute]) -> FieldDesc {
    let reflected = ReflectedScriptAttrs::from_hir(attrs);
    desc.attrs = reflected.attrs;
    desc.docs = reflected.docs;
    desc
}

fn apply_field_type_hint(desc: FieldDesc, type_hint: &Option<vela_hir::HirTypeHint>) -> FieldDesc {
    match type_hint {
        Some(hint) => desc.type_hint(hint.display()),
        None => desc,
    }
}

fn apply_field_type_hint_display(desc: FieldDesc, type_hint: &str) -> FieldDesc {
    if type_hint.is_empty() {
        desc
    } else {
        desc.type_hint(type_hint)
    }
}

fn apply_variant_attrs(mut desc: VariantDesc, attrs: &[vela_hir::HirAttribute]) -> VariantDesc {
    let reflected = ReflectedScriptAttrs::from_hir(attrs);
    desc.attrs = reflected.attrs;
    desc.docs = reflected.docs;
    desc
}

fn apply_trait_method_attrs(
    mut desc: TraitMethodDesc,
    signature: &vela_hir::FunctionSignature,
    attrs: &[vela_hir::HirAttribute],
) -> TraitMethodDesc {
    desc = apply_trait_method_signature(desc, signature);
    let reflected = ReflectedScriptAttrs::from_hir(attrs);
    desc.attrs = reflected.attrs;
    desc.docs = reflected.docs;
    desc
}

fn apply_trait_method_signature(
    mut desc: TraitMethodDesc,
    signature: &vela_hir::FunctionSignature,
) -> TraitMethodDesc {
    for param in &signature.params {
        let mut param_desc =
            MethodParamDesc::new(param.name.clone()).defaulted(param.default_value_span.is_some());
        if let Some(type_hint) = &param.type_hint {
            param_desc = param_desc.type_hint(type_hint.display());
        }
        desc = desc.param(param_desc);
    }
    if let Some(return_type) = &signature.return_type {
        desc = desc.return_type(return_type.display());
    }
    desc
}

struct VariantFieldMetadata {
    name: String,
    attrs: Vec<vela_hir::HirAttribute>,
    span: Span,
    type_hint: String,
    has_default: bool,
}

fn enum_variant_fields(fields: &EnumVariantFieldsHint) -> Vec<VariantFieldMetadata> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| VariantFieldMetadata {
                name: index.to_string(),
                attrs: Vec::new(),
                span: field.span,
                type_hint: field
                    .type_hint
                    .as_ref()
                    .map_or_else(String::new, vela_hir::HirTypeHint::display),
                has_default: field.default_value_span.is_some(),
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| VariantFieldMetadata {
                name: field.name.clone(),
                attrs: field.attrs.clone(),
                span: field.span,
                type_hint: field
                    .type_hint
                    .as_ref()
                    .map_or_else(String::new, vela_hir::HirTypeHint::display),
                has_default: field.default_value_span.is_some(),
            })
            .collect(),
    }
}

fn qualified_type_name(graph: &ModuleGraph, declaration: &Declaration) -> String {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    if module_path.segments().is_empty() {
        declaration.name.clone()
    } else {
        format!("{}.{}", module_path.join(), declaration.name)
    }
}

fn qualified_path_name(graph: &ModuleGraph, owner: &Declaration, path: &[String]) -> String {
    if path.len() != 1 {
        return path.join(".");
    }
    let Some(module_path) = graph.module_path(owner.module) else {
        return path[0].clone();
    };
    if module_path.segments().is_empty() {
        path[0].clone()
    } else {
        format!("{}.{}", module_path.join(), path[0])
    }
}

fn struct_schema_hash(type_name: &str, shape: &vela_hir::StructShape) -> SchemaHash {
    let members = shape
        .fields
        .iter()
        .map(|field| {
            (
                stable_field_id(type_name, &field.name).get(),
                field.name.clone(),
                field
                    .type_hint
                    .as_ref()
                    .map_or_else(String::new, vela_hir::HirTypeHint::display),
            )
        })
        .collect::<Vec<_>>();
    schema_hash("struct", type_name, members)
}

fn enum_schema_hash(type_name: &str, shape: &vela_hir::EnumShape) -> SchemaHash {
    let members = shape
        .variants
        .iter()
        .map(|variant| {
            (
                stable_variant_id(type_name, &variant.name).get(),
                variant.name.clone(),
                enum_variant_signature(type_name, variant),
            )
        })
        .collect::<Vec<_>>();
    schema_hash("enum", type_name, members)
}

fn enum_variant_signature(type_name: &str, variant: &vela_hir::EnumVariantHint) -> String {
    let variant_owner = enum_variant_owner(type_name, &variant.name);
    let mut fields = enum_variant_fields(&variant.fields)
        .into_iter()
        .map(|field| {
            (
                stable_field_id(&variant_owner, &field.name).get(),
                field.name,
                field.type_hint,
            )
        })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    fields
        .into_iter()
        .map(|(_, field_name, type_hint)| format!("{field_name}:{type_hint}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn enum_variant_owner(type_name: &str, variant: &str) -> String {
    format!("{type_name}.{variant}")
}

fn schema_hash(kind: &str, type_name: &str, mut members: Vec<(u32, String, String)>) -> SchemaHash {
    members.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let mut hash = 0xcbf2_9ce4_8422_2325;
    hash_bytes(&mut hash, kind.as_bytes());
    hash_bytes(&mut hash, &[0]);
    hash_bytes(&mut hash, type_name.as_bytes());
    hash_bytes(&mut hash, &[0]);
    for (member_id, member_name, type_hint) in members {
        hash_bytes(&mut hash, &member_id.to_le_bytes());
        hash_bytes(&mut hash, &[0]);
        hash_bytes(&mut hash, member_name.as_bytes());
        hash_bytes(&mut hash, &[0]);
        hash_bytes(&mut hash, type_hint.as_bytes());
        hash_bytes(&mut hash, &[0]);
    }
    SchemaHash::new(hash)
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn stable_type_id(name: &str) -> TypeId {
    TypeId::new(stable_id("type", name, ""))
}

fn stable_field_id(type_name: &str, field_name: &str) -> FieldId {
    FieldId::new(stable_id("field", type_name, field_name))
}

fn stable_variant_id(type_name: &str, variant_name: &str) -> VariantId {
    VariantId::new(stable_id("variant", type_name, variant_name))
}

fn stable_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(stable_id("trait_method", trait_name, method_name))
}

fn stable_id(kind: &str, owner: &str, member: &str) -> u32 {
    let mut hash = 0x811c_9dc5;
    for byte in kind
        .bytes()
        .chain([0])
        .chain(owner.bytes())
        .chain([0])
        .chain(member.bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    if hash == 0 { 1 } else { hash }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::SourceId;
    use vela_hir::{ModulePath, ModuleSource};

    #[test]
    fn registers_script_struct_and_enum_metadata_from_hir() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            r#"
#[doc("Reward metadata.")]
#[domain("gameplay")]
#[policy(level = 3, tags = ["reward", game.reward.Event])]
struct Reward {
    #[doc("Reward count.")]
    count: int = 1,
    item_id: string = "gold",
}

enum QuestProgress {
    None,
    #[active]
    Active { quest_id: string, count: int = 0 },
    Finished(quest_id: string),
}
"#,
        ));
        let mut registry = TypeRegistry::new();

        registry.register_script_types(&graph);

        let reward = registry
            .type_by_name("game.reward.Reward")
            .expect("Reward type metadata");
        let progress = registry
            .type_by_name("game.reward.QuestProgress")
            .expect("QuestProgress type metadata");
        assert_eq!(reward.kind, TypeKind::ScriptStruct);
        assert_eq!(progress.kind, TypeKind::ScriptEnum);
        assert!(reward.source_span.is_some());
        assert_eq!(
            reward.source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );
        assert!(progress.source_span.is_some());
        assert!(reward.schema_hash.is_some());
        assert!(progress.schema_hash.is_some());
        assert_eq!(reward.docs.as_deref(), Some("Reward metadata."));
        assert_eq!(reward.attrs.get("domain"), Some("gameplay"));
        assert_eq!(
            reward.attrs.get("policy"),
            Some("level=3,tags=[\"reward\",game.reward.Event]")
        );
        assert_eq!(
            reward
                .fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["count", "item_id"]
        );
        assert_eq!(
            reward
                .fields
                .iter()
                .find(|field| field.name == "count")
                .and_then(|field| field.docs.as_deref()),
            Some("Reward count.")
        );
        let count_field = reward
            .fields
            .iter()
            .find(|field| field.name == "count")
            .expect("count field");
        assert_eq!(count_field.type_hint.as_deref(), Some("int"));
        assert!(count_field.has_default);
        assert_eq!(
            count_field.source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );
        assert_eq!(
            progress
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            ["None", "Active", "Finished"]
        );
        let active = progress
            .variants
            .iter()
            .find(|variant| variant.name == "Active")
            .expect("Active variant");
        assert_eq!(active.attrs.get("active"), Some("true"));
        assert_eq!(
            active.source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );
        assert_eq!(
            active
                .fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["quest_id", "count"]
        );
        assert_eq!(
            active
                .fields
                .iter()
                .find(|field| field.name == "quest_id")
                .and_then(|field| field.type_hint.as_deref()),
            Some("string")
        );
        assert!(
            active
                .fields
                .iter()
                .find(|field| field.name == "count")
                .is_some_and(|field| field.has_default)
        );
        assert_eq!(
            active
                .fields
                .iter()
                .find(|field| field.name == "quest_id")
                .and_then(|field| field.source_span)
                .map(|span| span.source),
            Some(SourceId::new(1))
        );
        assert_eq!(
            reward
                .fields
                .iter()
                .find(|field| field.name == "count")
                .map(|field| field.id),
            Some(stable_field_id("game.reward.Reward", "count"))
        );
        assert_eq!(
            progress
                .variants
                .iter()
                .find(|variant| variant.name == "Active")
                .map(|variant| variant.id),
            Some(stable_variant_id("game.reward.QuestProgress", "Active"))
        );
    }

    #[test]
    fn script_type_member_ids_survive_reordering() {
        let mut first = ModuleGraph::new();
        first.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            "struct Reward { count, item_id }\nenum QuestProgress { None, Active }",
        ));
        let mut second = ModuleGraph::new();
        second.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            "struct Reward { item_id, count }\nenum QuestProgress { Active, None }",
        ));
        let mut first_registry = TypeRegistry::new();
        let mut second_registry = TypeRegistry::new();

        first_registry.register_script_types(&first);
        second_registry.register_script_types(&second);

        let first_reward = first_registry
            .type_by_name("game.reward.Reward")
            .expect("first Reward");
        let second_reward = second_registry
            .type_by_name("game.reward.Reward")
            .expect("second Reward");
        let first_progress = first_registry
            .type_by_name("game.reward.QuestProgress")
            .expect("first QuestProgress");
        let second_progress = second_registry
            .type_by_name("game.reward.QuestProgress")
            .expect("second QuestProgress");

        let first_count = first_reward
            .fields
            .iter()
            .find(|field| field.name == "count")
            .map(|field| field.id);
        let second_count = second_reward
            .fields
            .iter()
            .find(|field| field.name == "count")
            .map(|field| field.id);
        let first_active = first_progress
            .variants
            .iter()
            .find(|variant| variant.name == "Active")
            .map(|variant| variant.id);
        let second_active = second_progress
            .variants
            .iter()
            .find(|variant| variant.name == "Active")
            .map(|variant| variant.id);

        assert_eq!(first_count, second_count);
        assert_eq!(first_active, second_active);
        assert_eq!(first_reward.schema_hash, second_reward.schema_hash);
        assert_eq!(first_progress.schema_hash, second_progress.schema_hash);
    }

    #[test]
    fn script_type_schema_hash_changes_for_member_or_hint_changes() {
        let mut original = ModuleGraph::new();
        original.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            "struct Reward { count: int, item_id: string }\nenum QuestProgress { None, Active }",
        ));
        let mut changed = ModuleGraph::new();
        changed.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            "struct Reward { count: float, bonus: int }\nenum QuestProgress { None, Finished }",
        ));
        let mut original_registry = TypeRegistry::new();
        let mut changed_registry = TypeRegistry::new();

        original_registry.register_script_types(&original);
        changed_registry.register_script_types(&changed);

        let original_reward = original_registry
            .type_by_name("game.reward.Reward")
            .expect("original Reward");
        let changed_reward = changed_registry
            .type_by_name("game.reward.Reward")
            .expect("changed Reward");
        let original_progress = original_registry
            .type_by_name("game.reward.QuestProgress")
            .expect("original QuestProgress");
        let changed_progress = changed_registry
            .type_by_name("game.reward.QuestProgress")
            .expect("changed QuestProgress");

        assert_ne!(original_reward.schema_hash, changed_reward.schema_hash);
        assert_ne!(original_progress.schema_hash, changed_progress.schema_hash);
    }

    #[test]
    fn registers_script_traits_and_impls_from_hir() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.combat"),
            r#"
#[doc("Damage protocol.")]
trait Damageable {
    #[doc("Apply damage.")]
    fn damage(self, amount: int) -> int;
    fn alive(self) -> bool { return true; }
}

struct Player { hp: int }

impl Damageable for Player {
    fn damage(self, amount: int) -> int {
        return self.hp - amount;
    }
}
"#,
        ));
        let mut registry = TypeRegistry::new();

        registry.register_script_types(&graph);

        let damageable = registry
            .trait_by_name("game.combat.Damageable")
            .expect("Damageable trait");
        let player = registry
            .type_by_name("game.combat.Player")
            .expect("Player type");

        assert_eq!(damageable.docs.as_deref(), Some("Damage protocol."));
        assert_eq!(
            damageable.source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );
        assert_eq!(
            damageable
                .methods
                .iter()
                .map(|method| (method.name.as_str(), method.has_default))
                .collect::<Vec<_>>(),
            [("damage", false), ("alive", true)]
        );
        assert_eq!(damageable.methods[0].docs.as_deref(), Some("Apply damage."));
        assert_eq!(damageable.methods[0].return_type.as_deref(), Some("int"));
        assert_eq!(
            damageable.methods[0]
                .params
                .iter()
                .map(|param| (param.name.as_str(), param.type_hint.as_deref()))
                .collect::<Vec<_>>(),
            [("self", None), ("amount", Some("int"))]
        );
        assert_eq!(damageable.methods[1].return_type.as_deref(), Some("bool"));
        assert_eq!(
            damageable.methods[0].source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );
        assert_eq!(
            player
                .traits
                .iter()
                .map(|trait_desc| trait_desc.name.as_str())
                .collect::<Vec<_>>(),
            ["game.combat.Damageable"]
        );
        assert_eq!(player.traits[0].id, damageable.id);
        assert_eq!(player.traits[0].methods, damageable.methods);
    }
}
