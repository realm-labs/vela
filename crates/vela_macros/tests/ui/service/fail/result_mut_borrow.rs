use vela_macros::service;

pub struct Table;
pub struct Row;
pub struct ServiceError;

#[service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn row<'a>(
        &self,
        table: &'a mut Table,
    ) -> Result<&'a mut Row, ServiceError>;
}

fn main() {}
