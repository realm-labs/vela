use vela_macros::service;

pub struct Table;
pub struct Row;

#[service(path = "coverage::projected")]
pub trait ProjectedService: Send + Sync {
    fn row<'a>(&self, table: &'a Table) -> &'a Row;
}

fn main() {}
