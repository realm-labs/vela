//! Shared foundations for Vela crates.

pub mod diagnostic_render;
pub mod standard_ids;

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU32;

macro_rules! stable_id {
    ($name:ident, $inner:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(pub $inner);

        impl $name {
            #[must_use]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }
    };
}

stable_id!(GlobalSlot, usize);
stable_id!(HostMethodId, u64);
stable_id!(HostObjectId, u64);
stable_id!(HostTypeId, u64);
stable_id!(ShapeId, u32);
stable_id!(SourceId, u32);

#[must_use]
pub const fn stable_id(namespace: &str, owner: &str, name: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    stable_hash_bytes(&mut hash, namespace.as_bytes());
    stable_hash_bytes(&mut hash, &[0]);
    stable_hash_bytes(&mut hash, owner.as_bytes());
    stable_hash_bytes(&mut hash, &[0]);
    stable_hash_bytes(&mut hash, name.as_bytes());
    if hash == 0 { 1 } else { hash }
}

const fn stable_hash_bytes(hash: &mut u64, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        *hash ^= bytes[index] as u64;
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Symbol(NonZeroU32);

impl Symbol {
    #[must_use]
    pub const fn new(raw: NonZeroU32) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Debug, Default)]
pub struct SymbolInterner {
    strings: Vec<String>,
    symbols: HashMap<String, Symbol>,
}

impl SymbolInterner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, text: impl AsRef<str>) -> Symbol {
        let text = text.as_ref();
        if let Some(symbol) = self.symbols.get(text) {
            return *symbol;
        }

        let next_index = self
            .strings
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .expect("symbol table exceeded u32::MAX entries");
        let owned = text.to_owned();
        let symbol = Symbol::new(next_index);

        self.strings.push(owned.clone());
        self.symbols.insert(owned, symbol);

        symbol
    }

    #[must_use]
    pub fn resolve(&self, symbol: Symbol) -> Option<&str> {
        let index = usize::try_from(symbol.get()).ok()?.checked_sub(1)?;
        self.strings.get(index).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    pub source: SourceId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[must_use]
    pub const fn new(source: SourceId, start: u32, end: u32) -> Self {
        Self { source, start, end }
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub const fn contains(self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub span: Option<Span>,
    pub labels: Vec<Label>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            span: None,
            labels: Vec::new(),
        }
    }

    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: None,
            message: message.into(),
            span: None,
            labels: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    #[must_use]
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            Severity::Help => "help",
        };
        formatter.write_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_reuses_symbols_and_resolves_text() {
        let mut interner = SymbolInterner::new();

        let player = interner.intern("player");
        let level = interner.intern("level");
        let player_again = interner.intern("player");

        assert_eq!(player, player_again);
        assert_ne!(player, level);
        assert_eq!(interner.resolve(player), Some("player"));
        assert_eq!(interner.resolve(level), Some("level"));
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn span_tracks_source_offsets() {
        let span = Span::new(SourceId::new(7), 10, 15);

        assert_eq!(span.len(), 5);
        assert!(span.contains(10));
        assert!(span.contains(14));
        assert!(!span.contains(15));
        assert!(!span.is_empty());
    }

    #[test]
    fn diagnostic_builder_keeps_primary_span_and_labels() {
        let primary = Span::new(SourceId::new(1), 2, 8);
        let label = Span::new(SourceId::new(1), 4, 6);

        let diagnostic = Diagnostic::error("unknown field")
            .with_code("E0001")
            .with_span(primary)
            .with_label(label, "field lookup failed");

        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code.as_deref(), Some("E0001"));
        assert_eq!(diagnostic.span, Some(primary));
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.labels[0].span, label);
    }
}
