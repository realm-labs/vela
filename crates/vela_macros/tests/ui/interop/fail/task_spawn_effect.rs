use vela_macros::{export, export_module, methods};

#[export(path = "host::spawn", effects(task_spawn))]
pub fn spawn() {}

#[export_module(path = "host")]
mod exports {
    #[vela(effects(task_spawn))]
    pub fn spawn() {}
}

struct Table;

#[methods(path = "host::Table")]
impl Table {
    #[vela(effects(task_spawn))]
    pub fn spawn(&self) {}
}

fn main() {}
