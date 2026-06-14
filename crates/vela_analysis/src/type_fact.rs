use std::fmt;

use vela_common::PrimitiveTag;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeFact {
    Unknown,
    Never,
    Any,
    Primitive(PrimitiveTag),
    Range,
    Array {
        element: Box<TypeFact>,
    },
    Map {
        key: Box<TypeFact>,
        value: Box<TypeFact>,
    },
    Set {
        element: Box<TypeFact>,
    },
    Iterator {
        item: Box<TypeFact>,
    },
    Option {
        some: Box<TypeFact>,
    },
    OptionSome {
        some: Box<TypeFact>,
    },
    OptionNone,
    Result {
        ok: Box<TypeFact>,
        err: Box<TypeFact>,
    },
    ResultOk {
        ok: Box<TypeFact>,
    },
    ResultErr {
        err: Box<TypeFact>,
    },
    Function {
        params: Vec<TypeFact>,
        returns: Box<TypeFact>,
    },
    Record {
        name: String,
    },
    Enum {
        name: String,
        variant: Option<String>,
    },
    Host {
        name: String,
    },
    Trait {
        name: String,
    },
    Module {
        name: String,
    },
    Union(Vec<TypeFact>),
}

impl TypeFact {
    pub const NULL: Self = Self::Primitive(PrimitiveTag::Null);
    pub const BOOL: Self = Self::Primitive(PrimitiveTag::Bool);
    pub const CHAR: Self = Self::Primitive(PrimitiveTag::Char);
    pub const I8: Self = Self::Primitive(PrimitiveTag::I8);
    pub const I16: Self = Self::Primitive(PrimitiveTag::I16);
    pub const I32: Self = Self::Primitive(PrimitiveTag::I32);
    pub const I64: Self = Self::Primitive(PrimitiveTag::I64);
    pub const U8: Self = Self::Primitive(PrimitiveTag::U8);
    pub const U16: Self = Self::Primitive(PrimitiveTag::U16);
    pub const U32: Self = Self::Primitive(PrimitiveTag::U32);
    pub const U64: Self = Self::Primitive(PrimitiveTag::U64);
    pub const F32: Self = Self::Primitive(PrimitiveTag::F32);
    pub const F64: Self = Self::Primitive(PrimitiveTag::F64);
    pub const STRING: Self = Self::Primitive(PrimitiveTag::String);
    pub const BYTES: Self = Self::Primitive(PrimitiveTag::Bytes);

    pub const fn primitive(tag: PrimitiveTag) -> Self {
        Self::Primitive(tag)
    }

    pub fn array(element: TypeFact) -> Self {
        Self::Array {
            element: Box::new(element),
        }
    }

    pub fn map(key: TypeFact, value: TypeFact) -> Self {
        Self::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    pub fn set(element: TypeFact) -> Self {
        Self::Set {
            element: Box::new(element),
        }
    }

    pub fn iterator(item: TypeFact) -> Self {
        Self::Iterator {
            item: Box::new(item),
        }
    }

    pub fn option(some: TypeFact) -> Self {
        Self::Option {
            some: Box::new(some),
        }
    }

    pub fn option_some(some: TypeFact) -> Self {
        Self::OptionSome {
            some: Box::new(some),
        }
    }

    pub fn option_none() -> Self {
        Self::OptionNone
    }

    pub fn result(ok: TypeFact, err: TypeFact) -> Self {
        Self::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        }
    }

    pub fn result_ok(ok: TypeFact) -> Self {
        Self::ResultOk { ok: Box::new(ok) }
    }

    pub fn result_err(err: TypeFact) -> Self {
        Self::ResultErr { err: Box::new(err) }
    }

    pub fn function(params: Vec<TypeFact>, returns: TypeFact) -> Self {
        Self::Function {
            params,
            returns: Box::new(returns),
        }
    }

    pub fn record(name: impl Into<String>) -> Self {
        Self::Record { name: name.into() }
    }

    pub fn enum_type(name: impl Into<String>, variant: Option<impl Into<String>>) -> Self {
        Self::Enum {
            name: name.into(),
            variant: variant.map(Into::into),
        }
    }

    pub fn host(name: impl Into<String>) -> Self {
        Self::Host { name: name.into() }
    }

    pub fn trait_type(name: impl Into<String>) -> Self {
        Self::Trait { name: name.into() }
    }

    pub fn module(name: impl Into<String>) -> Self {
        Self::Module { name: name.into() }
    }

    pub fn union(facts: impl IntoIterator<Item = TypeFact>) -> Self {
        let mut merged = Vec::new();
        let mut saw_never = false;
        for fact in facts {
            match fact {
                Self::Union(facts) => {
                    for fact in facts {
                        push_unique_fact(&mut merged, fact, &mut saw_never);
                    }
                }
                fact => push_unique_fact(&mut merged, fact, &mut saw_never),
            }
        }

        match merged.as_slice() {
            [] if saw_never => Self::Never,
            [] => Self::Unknown,
            [fact] => fact.clone(),
            _ => Self::Union(merged),
        }
    }

    pub fn without_null(&self) -> Self {
        match self {
            Self::Primitive(PrimitiveTag::Null) => Self::Never,
            Self::Union(facts) => {
                let narrowed = facts
                    .iter()
                    .filter(|fact| !matches!(fact, Self::Primitive(PrimitiveTag::Null)))
                    .cloned()
                    .collect::<Vec<_>>();
                if narrowed.is_empty() {
                    Self::Never
                } else {
                    Self::union(narrowed)
                }
            }
            fact => fact.clone(),
        }
    }

    pub fn only_null(&self) -> Self {
        match self {
            Self::Primitive(PrimitiveTag::Null) | Self::Unknown | Self::Any => Self::NULL,
            Self::Union(facts)
                if facts
                    .iter()
                    .any(|fact| matches!(fact, Self::Primitive(PrimitiveTag::Null))) =>
            {
                Self::NULL
            }
            _ => Self::Never,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Unknown => "unknown".to_owned(),
            Self::Never => "never".to_owned(),
            Self::Any => "Any".to_owned(),
            Self::Primitive(PrimitiveTag::String) => "String".to_owned(),
            Self::Primitive(PrimitiveTag::Bytes) => "Bytes".to_owned(),
            Self::Primitive(tag) => tag.name().to_owned(),
            Self::Range => "Range".to_owned(),
            Self::Array { element } => format!("Array({})", element.display_name()),
            Self::Map { key, value } => {
                format!("Map({}, {})", key.display_name(), value.display_name())
            }
            Self::Set { element } => format!("Set({})", element.display_name()),
            Self::Iterator { .. } => "Iterator".to_owned(),
            Self::Option { some } => format!("Option({})", some.display_name()),
            Self::OptionSome { some } => format!("Option::Some({})", some.display_name()),
            Self::OptionNone => "Option::None".to_owned(),
            Self::Result { ok, err } => {
                format!("Result({}, {})", ok.display_name(), err.display_name())
            }
            Self::ResultOk { ok } => format!("Result::Ok({})", ok.display_name()),
            Self::ResultErr { err } => format!("Result::Err({})", err.display_name()),
            Self::Function { params, returns } => {
                let params = params
                    .iter()
                    .map(Self::display_name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Function({params}) -> {}", returns.display_name())
            }
            Self::Record { name }
            | Self::Host { name }
            | Self::Trait { name }
            | Self::Module { name } => name.clone(),
            Self::Enum {
                name,
                variant: Some(variant),
            } => format!("{name}::{variant}"),
            Self::Enum {
                name,
                variant: None,
            } => name.clone(),
            Self::Union(facts) => facts
                .iter()
                .map(Self::display_name)
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }
}

fn push_unique_fact(facts: &mut Vec<TypeFact>, fact: TypeFact, saw_never: &mut bool) {
    if matches!(fact, TypeFact::Never) {
        *saw_never = true;
        return;
    }
    if !facts.contains(&fact) {
        facts.push(fact);
    }
}

impl fmt::Display for TypeFact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_avoid_script_generic_syntax() {
        let fact = TypeFact::map(TypeFact::STRING, TypeFact::array(TypeFact::I64));

        assert_eq!(fact.display_name(), "Map(String, Array(i64))");
        assert_eq!(
            TypeFact::option_some(TypeFact::I64).display_name(),
            "Option::Some(i64)"
        );
        assert_eq!(
            TypeFact::result_err(TypeFact::STRING).display_name(),
            "Result::Err(String)"
        );
        assert_eq!(TypeFact::iterator(TypeFact::I64).display_name(), "Iterator");
        assert!(!fact.display_name().contains('<'));
        assert!(!fact.display_name().contains('>'));
    }

    #[test]
    fn union_flattens_and_deduplicates_facts() {
        let fact = TypeFact::union([
            TypeFact::I64,
            TypeFact::Union(vec![TypeFact::STRING, TypeFact::I64]),
        ]);

        assert_eq!(fact, TypeFact::Union(vec![TypeFact::I64, TypeFact::STRING]));
    }

    #[test]
    fn null_narrowing_removes_or_selects_null_from_unions() {
        let fact = TypeFact::Union(vec![TypeFact::NULL, TypeFact::host("Player")]);

        assert_eq!(fact.without_null(), TypeFact::host("Player"));
        assert_eq!(fact.only_null(), TypeFact::NULL);
        assert_eq!(TypeFact::NULL.without_null(), TypeFact::Never);
        assert_eq!(TypeFact::I64.only_null(), TypeFact::Never);
    }

    #[test]
    fn module_facts_display_as_module_paths() {
        let fact = TypeFact::module("game::reward");

        assert_eq!(fact.display_name(), "game::reward");
    }
}
