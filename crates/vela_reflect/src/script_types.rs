use vela_common::{FieldId, TypeId, VariantId};
use vela_hir::{Declaration, DeclarationKind, ModuleGraph};

use crate::{FieldDesc, TypeDesc, TypeKey, TypeRegistry, VariantDesc};

impl TypeRegistry {
    pub fn register_script_types(&mut self, graph: &ModuleGraph) {
        for declaration in graph.declarations() {
            match declaration.kind {
                DeclarationKind::Struct => {
                    let Some(shape) = graph.struct_shape(declaration.id) else {
                        continue;
                    };
                    let type_name = qualified_type_name(graph, declaration);
                    let desc = shape.fields.iter().fold(
                        TypeDesc::new(TypeKey::new(stable_type_id(&type_name), type_name.clone())),
                        |desc, field| {
                            desc.field(FieldDesc::new(
                                stable_field_id(&type_name, &field.name),
                                field.name.clone(),
                            ))
                        },
                    );
                    self.register(desc);
                }
                DeclarationKind::Enum => {
                    let Some(shape) = graph.enum_shape(declaration.id) else {
                        continue;
                    };
                    let type_name = qualified_type_name(graph, declaration);
                    let desc = shape.variants.iter().fold(
                        TypeDesc::new(TypeKey::new(stable_type_id(&type_name), type_name.clone())),
                        |desc, variant| {
                            desc.variant(VariantDesc::new(
                                stable_variant_id(&type_name, &variant.name),
                                variant.name.clone(),
                            ))
                        },
                    );
                    self.register(desc);
                }
                DeclarationKind::Const
                | DeclarationKind::Function
                | DeclarationKind::Trait
                | DeclarationKind::Impl => {}
            }
        }
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

fn stable_type_id(name: &str) -> TypeId {
    TypeId::new(stable_id("type", name, ""))
}

fn stable_field_id(type_name: &str, field_name: &str) -> FieldId {
    FieldId::new(stable_id("field", type_name, field_name))
}

fn stable_variant_id(type_name: &str, variant_name: &str) -> VariantId {
    VariantId::new(stable_id("variant", type_name, variant_name))
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
struct Reward {
    count: int,
    item_id: string,
}

enum QuestProgress {
    None,
    Active,
    Finished,
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
        assert_eq!(
            reward
                .fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["count", "item_id"]
        );
        assert_eq!(
            progress
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            ["None", "Active", "Finished"]
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
    }
}
