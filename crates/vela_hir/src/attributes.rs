use vela_syntax::ast::Attribute;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAttribute {
    pub name: String,
    pub value: Option<String>,
}

impl HirAttribute {
    #[must_use]
    pub fn from_syntax(attribute: &Attribute) -> Self {
        Self {
            name: attribute.path.join("."),
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
