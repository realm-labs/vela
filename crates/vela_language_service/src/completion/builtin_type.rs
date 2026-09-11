use vela_analysis::type_fact::TypeFact;

use super::{CompletionInsertFormat, CompletionItem, CompletionKind, display_type_detail_parts};
use crate::symbol_ref::builtin_symbol;

pub(super) fn builtin_type_hint_completions() -> Vec<CompletionItem> {
    [
        ("()", TypeFact::UNIT),
        ("bool", TypeFact::BOOL),
        ("char", TypeFact::CHAR),
        ("i8", TypeFact::I8),
        ("i16", TypeFact::I16),
        ("i32", TypeFact::I32),
        ("i64", TypeFact::I64),
        ("u8", TypeFact::U8),
        ("u16", TypeFact::U16),
        ("u32", TypeFact::U32),
        ("u64", TypeFact::U64),
        ("f32", TypeFact::F32),
        ("f64", TypeFact::F64),
        ("Any", TypeFact::Any),
        ("Range", TypeFact::Range),
        ("Closure", TypeFact::Closure),
        ("String", TypeFact::STRING),
        ("Bytes", TypeFact::BYTES),
        ("Array", TypeFact::array(TypeFact::Unknown)),
        ("Map", TypeFact::map(TypeFact::Unknown, TypeFact::Unknown)),
        ("Set", TypeFact::set(TypeFact::Unknown)),
        ("Iterator", TypeFact::iterator(TypeFact::Unknown)),
        ("Option", TypeFact::option(TypeFact::Unknown)),
        (
            "Result",
            TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        ),
    ]
    .into_iter()
    .map(|(label, fact)| (label, fact.display_name()))
    // Function is an erased callable contract, not a zero-argument signature.
    .chain([("Function", "Function".to_owned())])
    .map(|(label, detail)| {
        let detail_parts = display_type_detail_parts(detail);
        CompletionItem {
            label: label.to_owned(),
            kind: CompletionKind::Type,
            detail: detail_parts.render(),
            insert_text: Some(label.to_owned()),
            insert_format: CompletionInsertFormat::PlainText,
            sort_text: None,
            metadata: Default::default(),
        }
        .with_detail_parts(detail_parts)
        .with_symbol(builtin_symbol(label))
    })
    .collect()
}
