use std::collections::BTreeSet;

use vela_common::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAttribute {
    pub name: String,
    pub arguments: Vec<HirAttributeArgument>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAttributeArgument {
    pub name: Option<String>,
    pub name_span: Option<Span>,
    pub value: HirAttributeValue,
    pub span: Span,
    pub value_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirAttributeValue {
    String(String),
    Bool(bool),
    Integer(String),
    Float(String),
    Path(Vec<String>),
    Array(Vec<HirAttributeValue>),
    Map(Vec<HirAttributeMapEntry>),
    Raw(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAttributeMapEntry {
    pub key: String,
    pub key_span: Span,
    pub value: HirAttributeValue,
    pub span: Span,
    pub value_span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaIdAttrError {
    MissingValue,
    InvalidValue,
    Zero,
}

impl HirAttribute {
    #[must_use]
    pub fn positional_argument(&self) -> Option<&HirAttributeArgument> {
        self.arguments
            .iter()
            .find(|argument| argument.name.is_none())
    }

    #[must_use]
    pub fn string_value(&self) -> String {
        if self.arguments.is_empty() {
            return "true".to_owned();
        }
        self.arguments
            .iter()
            .map(HirAttributeArgument::display)
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl HirAttributeArgument {
    #[must_use]
    pub fn display(&self) -> String {
        match &self.name {
            Some(name) => format!("{name}={}", self.value.display()),
            None => self.value.display(),
        }
    }
}

impl HirAttributeValue {
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::String(value) | Self::Integer(value) | Self::Float(value) | Self::Raw(value) => {
                value.clone()
            }
            Self::Bool(value) => value.to_string(),
            Self::Path(segments) => segments.join("::"),
            Self::Array(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(Self::display_nested)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::Map(entries) => format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|entry| format!("{}:{}", entry.key, entry.value.display_nested()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    #[must_use]
    pub const fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    fn display_nested(&self) -> String {
        match self {
            Self::String(value) => format!("\"{value}\""),
            _ => self.display(),
        }
    }
}

#[must_use]
pub fn derived_traits(attrs: &[HirAttribute]) -> BTreeSet<String> {
    attrs
        .iter()
        .filter(|attr| attr.name == "derive")
        .flat_map(|attr| attr.arguments.iter())
        .flat_map(|argument| {
            argument
                .value
                .display()
                .split(',')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .map(|trait_name| trait_name.trim().to_owned())
        .filter(|trait_name| !trait_name.is_empty())
        .collect()
}

#[must_use]
pub fn schema_id_attr(attrs: &[HirAttribute]) -> Option<u64> {
    attrs
        .iter()
        .find_map(|attr| parse_schema_id_attr(attr).unwrap_or_default())
}

pub fn parse_schema_id_attr(attr: &HirAttribute) -> Result<Option<u64>, SchemaIdAttrError> {
    if attr.name != "id" {
        return Ok(None);
    }
    let Some(argument) = attr.arguments.first() else {
        return Err(SchemaIdAttrError::MissingValue);
    };
    if attr.arguments.len() != 1 || argument.name.is_some() {
        return Err(SchemaIdAttrError::InvalidValue);
    }
    let HirAttributeValue::Integer(value) = &argument.value else {
        return Err(SchemaIdAttrError::InvalidValue);
    };
    let id = value
        .parse::<u64>()
        .map_err(|_| SchemaIdAttrError::InvalidValue)?;
    if id == 0 {
        return Err(SchemaIdAttrError::Zero);
    }
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_traits_read_structured_positional_arguments() {
        let attrs = [HirAttribute {
            name: "derive".to_owned(),
            arguments: vec![
                argument(HirAttributeValue::Path(vec!["PartialEq".to_owned()])),
                argument(HirAttributeValue::Path(vec!["Eq".to_owned()])),
                argument(HirAttributeValue::Path(vec!["PartialOrd".to_owned()])),
                argument(HirAttributeValue::Path(vec!["Ord".to_owned()])),
            ],
            span: span(),
        }];

        assert_eq!(
            derived_traits(&attrs),
            BTreeSet::from([
                "Eq".to_owned(),
                "Ord".to_owned(),
                "PartialEq".to_owned(),
                "PartialOrd".to_owned(),
            ])
        );
    }

    fn argument(value: HirAttributeValue) -> HirAttributeArgument {
        HirAttributeArgument {
            name: None,
            name_span: None,
            value,
            span: span(),
            value_span: span(),
        }
    }

    fn span() -> Span {
        Span::new(vela_common::SourceId::new(1), 0, 0)
    }
}
