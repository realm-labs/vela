use super::*;
use crate::ast::{BinaryOp, ExprKind, Literal, StmtKind};
use crate::lexer::lex;
use crate::token::{Keyword, Symbol, TokenKind};
use std::fmt::Write as _;

fn source_id() -> SourceId {
    SourceId::new(1)
}

fn param_names(params: &[Param]) -> Vec<String> {
    params.iter().map(|param| param.name.clone()).collect()
}

fn struct_field_names(fields: &[StructField]) -> Vec<String> {
    fields.iter().map(|field| field.name.clone()).collect()
}

fn enum_variant_names(variants: &[EnumVariant]) -> Vec<String> {
    variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect()
}

fn trait_method_names(methods: &[TraitMethod]) -> Vec<String> {
    methods.iter().map(|method| method.name.clone()).collect()
}

mod items;
mod lexer;
mod snapshots;
mod statements_and_expressions;
mod types_and_schema;
