use vela_macros::service;

pub struct Table;
pub struct Row;

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn row<'a>(&self, table: &'a Table) -> (&'a Row, i64);
}

fn main() {}
