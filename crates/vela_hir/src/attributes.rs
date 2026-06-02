use vela_syntax::ast::Attribute;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAttribute {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaIdAttrError {
    MissingValue,
    InvalidValue,
    Zero,
}

impl HirAttribute {
    #[must_use]
    pub fn from_syntax(attribute: &Attribute) -> Self {
        Self {
            name: attribute.path.join("::"),
            value: attribute.value.clone(),
        }
    }

    #[must_use]
    pub fn string_value(&self) -> &str {
        self.value.as_deref().unwrap_or("true")
    }
}

#[must_use]
pub fn attrs_from_syntax(attributes: &[Attribute]) -> Vec<HirAttribute> {
    attributes.iter().map(HirAttribute::from_syntax).collect()
}

#[must_use]
pub fn schema_id_attr(attrs: &[HirAttribute]) -> Option<u64> {
    attrs.iter().find_map(|attr| {
        parse_schema_id_attr(&attr.name, attr.value.as_deref()).unwrap_or_default()
    })
}

pub fn parse_schema_id_attr(
    name: &str,
    value: Option<&str>,
) -> Result<Option<u64>, SchemaIdAttrError> {
    if name != "id" {
        return Ok(None);
    }
    let Some(value) = value else {
        return Err(SchemaIdAttrError::MissingValue);
    };
    let id = value
        .parse::<u64>()
        .map_err(|_| SchemaIdAttrError::InvalidValue)?;
    if id == 0 {
        return Err(SchemaIdAttrError::Zero);
    }
    Ok(Some(id))
}
