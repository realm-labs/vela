fn type_with_reload_method(method: MethodDesc) -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
        .host_type(HostTypeId::new(1))
        .method(method)
}

fn assert_host_method_patch(tx: &PatchTx, _method: HostMethodId, _amount: i64) {
    assert_eq!(tx.mutation_count(), 1);
}

fn write_host_method_reward_modules(
    root: &std::path::Path,
    method_name: &str,
    reward: i64,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main(player: Player) {
    return grant(player);
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_host_method_reward_module(&reward_file, method_name, reward);
    reward_file
}

fn write_host_method_reward_module(path: &std::path::Path, method_name: &str, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant(player: Player) {{
    player.{method_name}(7);
    return {reward};
}}
"#
        ),
    )
    .expect("write host method reward module");
}

fn write_typed_reward_modules(
    root: &std::path::Path,
    main_return: &str,
    return_type: &str,
    reward_expr: &str,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        format!(
            r#"
use game::reward::grant

fn main() {{
    {main_return}
}}
"#
        ),
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_typed_reward_module(&reward_file, return_type, reward_expr);
    reward_file
}

fn write_typed_reward_module(path: &std::path::Path, return_type: &str, reward_expr: &str) {
    write_reward_module_with_signature(path, &format!("() -> {return_type}"), reward_expr);
}

fn write_reward_module_with_signature(path: &std::path::Path, signature: &str, reward_expr: &str) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant{signature} {{
    return {reward_expr};
}}
"#
        ),
    )
    .expect("write reward module with signature");
}

fn write_reward_module_with_helper(path: &std::path::Path, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return {reward};
}}

fn helper() {{
    return 1;
}}
"#
        ),
    )
    .expect("write reward module with helper");
}

fn write_reward_module_calling_helper(path: &std::path::Path, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return helper();
}}

fn helper() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write reward module calling helper");
}

fn write_reward_module_calling_public_helper(path: &std::path::Path, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return helper();
}}

pub fn helper() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write reward module calling public helper");
}
