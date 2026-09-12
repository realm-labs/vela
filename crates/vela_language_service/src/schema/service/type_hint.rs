use vela_common::CollectionViewMutation;
use vela_registry::TypeHintDef;

// Service metadata spells tuple and mutation capabilities explicitly. Parse
// with the registry grammar, then normalize those wire shapes to its typed IR.
pub(super) fn service_type_hint(text: &str) -> Option<TypeHintDef> {
    normalize(TypeHintDef::parse(text)?)
}

fn normalize(mut hint: TypeHintDef) -> Option<TypeHintDef> {
    let name = hint.path.join("::");
    if name == "Tuple" {
        if hint.args.len() < 2 {
            return None;
        }
        hint.path = vec!["()".to_owned()];
    }
    let type_arity = match name.as_str() {
        "ArrayMut" | "SetMut" => Some(1),
        "MapMut" => Some(2),
        _ => None,
    };
    if let Some(arity) = type_arity
        && hint.args.len() > arity
    {
        if hint.args.len() != arity + 1 {
            return None;
        }
        let mutation = hint.args.pop()?;
        if !mutation.args.is_empty() {
            return None;
        }
        hint.collection_mutation = Some(match mutation.path.as_slice() {
            [name] if name == "fixed" => CollectionViewMutation::Fixed,
            [name] if name == "growable" => CollectionViewMutation::Growable,
            _ => return None,
        });
    }
    hint.args = hint
        .args
        .into_iter()
        .map(normalize)
        .collect::<Option<_>>()?;
    Some(hint)
}
