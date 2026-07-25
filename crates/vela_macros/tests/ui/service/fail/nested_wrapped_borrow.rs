use vela_macros::service;

pub struct Table;
pub struct Row;
pub struct ServiceError;

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn rows<'a>(
        &self,
        table: &'a Table,
    ) -> Option<Result<Vec<&'a Row>, ServiceError>>;
}

fn main() {}
