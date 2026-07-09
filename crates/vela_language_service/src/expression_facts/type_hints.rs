use vela_analysis::type_fact::TypeFact;
use vela_common::PrimitiveTag;
use vela_syntax::ast::SyntaxTypeHint;

pub(super) fn type_fact_from_syntax_hint(hint: &SyntaxTypeHint) -> TypeFact {
    if hint.is_unit() {
        return TypeFact::UNIT;
    }
    let tuple_elements = hint.tuple_element_hints().collect::<Vec<_>>();
    if hint.is_tuple() {
        return TypeFact::tuple(tuple_elements.iter().map(type_fact_from_syntax_hint));
    }
    if hint.l_paren_token().is_some() && tuple_elements.len() == 1 {
        return type_fact_from_syntax_hint(&tuple_elements[0]);
    }
    let args = hint
        .type_arg_list()
        .map(|args| args.type_hints().collect::<Vec<_>>())
        .unwrap_or_default();
    match hint.path_segments().as_slice() {
        [name] => {
            if name == "Array" && args.len() == 1 {
                return TypeFact::array(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Map" && args.len() == 2 {
                return TypeFact::map(
                    type_fact_from_syntax_hint(&args[0]),
                    type_fact_from_syntax_hint(&args[1]),
                );
            }
            if name == "Set" && args.len() == 1 {
                return TypeFact::set(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Iterator" && args.len() == 1 {
                return TypeFact::iterator(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Option" && args.len() == 1 {
                return TypeFact::option(type_fact_from_syntax_hint(&args[0]));
            }
            if name == "Result" && args.len() == 2 {
                return TypeFact::result(
                    type_fact_from_syntax_hint(&args[0]),
                    type_fact_from_syntax_hint(&args[1]),
                );
            }
            if let Some(tag) = PrimitiveTag::from_name(name) {
                return TypeFact::primitive(tag);
            }

            match name.as_str() {
                "Any" => TypeFact::Any,
                "String" => TypeFact::primitive(PrimitiveTag::String),
                "Bytes" => TypeFact::primitive(PrimitiveTag::Bytes),
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
        path => TypeFact::record(path.join("::")),
    }
}
