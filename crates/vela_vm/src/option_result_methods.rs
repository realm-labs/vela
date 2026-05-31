mod access;
mod callbacks;
mod predicates;
mod simple;

pub(crate) use callbacks::{and_then, filter, map, map_err, or_else};
pub(crate) use predicates::{is_option, is_option_or_result, is_result};
pub(crate) use simple::{
    flatten, is_err, is_none, is_ok, is_some, ok_or, to_error_option, to_option, unwrap_or,
};
