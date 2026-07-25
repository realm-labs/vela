use vela_macros::service;

pub struct Table;
pub struct Row;

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn rows<'a>(
        &self,
        table: &'a Table,
    ) -> std::collections::BTreeMap<i64, Vec<Option<&'a Row>>>;
}

fn main() {}
