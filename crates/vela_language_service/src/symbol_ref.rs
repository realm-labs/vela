#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SymbolRef {
    Source(String),
    Schema(String),
    Builtin(String),
    Local(String),
}
