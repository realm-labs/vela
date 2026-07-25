use vela_macros::service;

pub struct Table;
pub struct Row;

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn row<'a>(&self, table: &'a mut Table) -> Option<&'a mut Row>;
}

fn main() {}
