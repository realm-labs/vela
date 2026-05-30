use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeFact {
    Unknown,
    Never,
    Any,
    Null,
    Bool,
    Int,
    Float,
    String,
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
            Self::Null => Self::Never,
            Self::Union(facts) => {
                let narrowed = facts
                    .iter()
                    .filter(|fact| !matches!(fact, Self::Null))
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
            Self::Null | Self::Unknown | Self::Any => Self::Null,
            Self::Union(facts) if facts.iter().any(|fact| matches!(fact, Self::Null)) => Self::Null,
            _ => Self::Never,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Unknown => "unknown".to_owned(),
            Self::Never => "never".to_owned(),
            Self::Any => "any".to_owned(),
            Self::Null => "null".to_owned(),
            Self::Bool => "bool".to_owned(),
            Self::Int => "int".to_owned(),
            Self::Float => "float".to_owned(),
            Self::String => "string".to_owned(),
            Self::Range => "range".to_owned(),
            Self::Array { element } => format!("array({})", element.display_name()),
            Self::Map { key, value } => {
                format!("map({}, {})", key.display_name(), value.display_name())
            }
            Self::Set { element } => format!("set({})", element.display_name()),
            Self::Option { some } => format!("Option({})", some.display_name()),
            Self::OptionSome { some } => format!("Option.Some({})", some.display_name()),
            Self::OptionNone => "Option.None".to_owned(),
            Self::Result { ok, err } => {
                format!("Result({}, {})", ok.display_name(), err.display_name())
            }
            Self::ResultOk { ok } => format!("Result.Ok({})", ok.display_name()),
            Self::ResultErr { err } => format!("Result.Err({})", err.display_name()),
            Self::Function { params, returns } => {
                let params = params
                    .iter()
                    .map(Self::display_name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({params}) -> {}", returns.display_name())
            }
            Self::Record { name }
            | Self::Host { name }
            | Self::Trait { name }
            | Self::Module { name } => name.clone(),
            Self::Enum {
                name,
                variant: Some(variant),
            } => format!("{name}.{variant}"),
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
        let fact = TypeFact::map(TypeFact::String, TypeFact::array(TypeFact::Int));

        assert_eq!(fact.display_name(), "map(string, array(int))");
        assert_eq!(
            TypeFact::option_some(TypeFact::Int).display_name(),
            "Option.Some(int)"
        );
        assert_eq!(
            TypeFact::result_err(TypeFact::String).display_name(),
            "Result.Err(string)"
        );
        assert!(!fact.display_name().contains('<'));
        assert!(!fact.display_name().contains('>'));
    }

    #[test]
    fn union_flattens_and_deduplicates_facts() {
        let fact = TypeFact::union([
            TypeFact::Int,
            TypeFact::Union(vec![TypeFact::String, TypeFact::Int]),
        ]);

        assert_eq!(fact, TypeFact::Union(vec![TypeFact::Int, TypeFact::String]));
    }

    #[test]
    fn null_narrowing_removes_or_selects_null_from_unions() {
        let fact = TypeFact::Union(vec![TypeFact::Null, TypeFact::host("Player")]);

        assert_eq!(fact.without_null(), TypeFact::host("Player"));
        assert_eq!(fact.only_null(), TypeFact::Null);
        assert_eq!(TypeFact::Null.without_null(), TypeFact::Never);
        assert_eq!(TypeFact::Int.only_null(), TypeFact::Never);
    }

    #[test]
    fn module_facts_display_as_module_paths() {
        let fact = TypeFact::module("game.reward");

        assert_eq!(fact.display_name(), "game.reward");
    }
}
