use vela_macros::{ScriptHost, ScriptReflect, methods};

#[derive(Debug, ScriptHost, ScriptReflect)]
#[vela(path = "game::Player")]
pub struct Player {
    #[vela(get, set)]
    pub level: i64,
}

#[methods]
impl Player {
    pub fn current_level(&self) -> i64 {
        self.level
    }
}
