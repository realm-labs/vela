use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use vela_macros::export;

#[export(path = "removed::vec")]
pub fn vec(_values: &Vec<i64>) {}

#[export(path = "removed::slice")]
pub fn slice(_values: &[i64]) {}

#[export(path = "removed::array")]
pub fn array(_values: &[i64; 2]) {}

#[export(path = "removed::map")]
pub fn map(_values: &BTreeMap<String, i64>) {}

#[export(path = "removed::hash_map")]
pub fn hash_map(_values: &HashMap<String, i64>) {}

#[export(path = "removed::set")]
pub fn set(_values: &BTreeSet<String>) {}

#[export(path = "removed::hash_set")]
pub fn hash_set(_values: &HashSet<String>) {}

#[export(path = "removed::mutable")]
pub fn mutable(_values: &mut Vec<i64>) {}

fn main() {}
