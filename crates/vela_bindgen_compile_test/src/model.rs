use vela_macros::{ScriptHost, ScriptReflect, methods};

#[derive(Debug, ScriptHost, ScriptReflect)]
#[script(path = "game::Player")]
pub struct Player {
    #[script(get, set)]
    pub level: i64,
}

#[methods]
impl Player {
    pub fn current_level(&self) -> i64 {
        self.level
    }
}
