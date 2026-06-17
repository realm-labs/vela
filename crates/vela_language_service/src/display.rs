use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct DisplayParts {
    parts: Vec<DisplayPart>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DisplayPart {
    kind: DisplayPartKind,
    text: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DisplayPartKind {
    Text,
    Symbol,
    Type,
    Parameter,
    Punctuation,
    Operator,
}

impl DisplayParts {
    #[must_use]
    pub const fn new() -> Self {
        Self { parts: Vec::new() }
    }

    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        let mut parts = Self::new();
        parts.push(DisplayPartKind::Text, text);
        parts
    }

    #[must_use]
    pub fn parameter(name: &str, type_name: &str) -> Self {
        let mut parts = Self::new();
        parts.push(DisplayPartKind::Parameter, name);
        parts.push(DisplayPartKind::Punctuation, ":");
        parts.push(DisplayPartKind::Text, " ");
        parts.push(DisplayPartKind::Type, type_name);
        parts
    }

    #[must_use]
    pub fn parameter_hint(name: &str) -> Self {
        let mut parts = Self::new();
        parts.push(DisplayPartKind::Parameter, name);
        parts.push(DisplayPartKind::Punctuation, ":");
        parts
    }

    #[must_use]
    pub fn type_annotation(type_name: &str) -> Self {
        let mut parts = Self::new();
        parts.push(DisplayPartKind::Punctuation, ":");
        parts.push(DisplayPartKind::Text, " ");
        parts.push(DisplayPartKind::Type, type_name);
        parts
    }

    #[must_use]
    pub fn callable_signature(
        name: &str,
        parameters: impl IntoIterator<Item = DisplayParts>,
        returns: Option<&str>,
    ) -> Self {
        let mut parts = Self::new();
        parts.push(DisplayPartKind::Symbol, name);
        parts.push(DisplayPartKind::Punctuation, "(");
        let mut first = true;
        for parameter in parameters {
            if first {
                first = false;
            } else {
                parts.push(DisplayPartKind::Punctuation, ",");
                parts.push(DisplayPartKind::Text, " ");
            }
            parts.extend(parameter);
        }
        parts.push(DisplayPartKind::Punctuation, ")");
        if let Some(returns) = returns {
            parts.push(DisplayPartKind::Text, " ");
            parts.push(DisplayPartKind::Operator, "->");
            parts.push(DisplayPartKind::Text, " ");
            parts.push(DisplayPartKind::Type, returns);
        }
        parts
    }

    pub fn push(&mut self, kind: DisplayPartKind, text: impl Into<String>) {
        self.parts.push(DisplayPart {
            kind,
            text: text.into(),
        });
    }

    pub fn extend(&mut self, other: DisplayParts) {
        self.parts.extend(other.parts);
    }

    #[must_use]
    pub fn parts(&self) -> &[DisplayPart] {
        &self.parts
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut text = String::new();
        for part in &self.parts {
            text.push_str(part.text());
        }
        text
    }
}

impl DisplayPart {
    #[must_use]
    pub const fn kind(&self) -> DisplayPartKind {
        self.kind
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for DisplayParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callable_signature_renders_structured_parts() {
        let label = DisplayParts::callable_signature(
            "grant",
            [
                DisplayParts::parameter("player", "Player"),
                DisplayParts::parameter("amount", "i64"),
            ],
            Some("bool"),
        );

        assert_eq!(label.render(), "grant(player: Player, amount: i64) -> bool");
        assert_eq!(label.parts()[0].kind(), DisplayPartKind::Symbol);
    }

    #[test]
    fn inlay_labels_render_existing_text_shape() {
        assert_eq!(DisplayParts::parameter_hint("amount").render(), "amount:");
        assert_eq!(DisplayParts::type_annotation("i64").render(), ": i64");
    }
}
