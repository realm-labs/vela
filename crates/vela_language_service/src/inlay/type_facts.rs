use vela_analysis::{registry::RegistryFacts, type_fact::TypeFact};
use vela_common::PrimitiveTag;
use vela_syntax::ast::SyntaxTypeHint;

pub(super) fn syntax_type_fact_from_hint(
    hint: &SyntaxTypeHint,
    schema: &RegistryFacts,
) -> TypeFact {
    let args = hint
        .type_arg_list()
        .map(|args| args.type_hints().collect::<Vec<_>>())
        .unwrap_or_default();
    match hint.path_segments().as_slice() {
        [name] => {
            if args.is_empty()
                && let Some(fact) = schema.type_fact(name).or_else(|| schema.trait_fact(name))
            {
                return fact.clone();
            }
            if name == "Array" && args.len() == 1 {
                return TypeFact::array(syntax_type_fact_from_hint(&args[0], schema));
            }
            if name == "Map" && args.len() == 2 {
                return TypeFact::map(
                    syntax_type_fact_from_hint(&args[0], schema),
                    syntax_type_fact_from_hint(&args[1], schema),
                );
            }
            if name == "Set" && args.len() == 1 {
                return TypeFact::set(syntax_type_fact_from_hint(&args[0], schema));
            }
            if name == "Iterator" && args.len() == 1 {
                return TypeFact::iterator(syntax_type_fact_from_hint(&args[0], schema));
            }
            if name == "Option" && args.len() == 1 {
                return TypeFact::option(syntax_type_fact_from_hint(&args[0], schema));
            }
            if name == "Result" && args.len() == 2 {
                return TypeFact::result(
                    syntax_type_fact_from_hint(&args[0], schema),
                    syntax_type_fact_from_hint(&args[1], schema),
                );
            }
            if let Some(tag) = PrimitiveTag::from_name(name) {
                return TypeFact::primitive(tag);
            }
            match name.as_str() {
                "Any" => TypeFact::Any,
                "String" => TypeFact::STRING,
                "Bytes" => TypeFact::BYTES,
                "Array" => TypeFact::array(TypeFact::Unknown),
                "Map" => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
                "Set" => TypeFact::set(TypeFact::Unknown),
                "Iterator" => TypeFact::iterator(TypeFact::Unknown),
                "Function" => TypeFact::function(Vec::new(), TypeFact::Unknown),
                "Option" => TypeFact::option(TypeFact::Unknown),
                "Result" => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
                name => TypeFact::record(name),
            }
        }
        path => {
            let qualified = path.join("::");
            schema
                .type_fact(&qualified)
                .or_else(|| schema.trait_fact(&qualified))
                .cloned()
                .unwrap_or_else(|| TypeFact::record(qualified))
        }
    }
}
