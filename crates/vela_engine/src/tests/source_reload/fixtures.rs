struct TestDir(std::path::PathBuf);

impl TestDir {
    fn join(&self, path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
        self.0.join(path)
    }
}

impl AsRef<std::path::Path> for TestDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl std::ops::Deref for TestDir {
    type Target = std::path::Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_test_dir(name: &str) -> TestDir {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vela_engine_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    TestDir(path)
}

fn runtime_from_hot_reload_source(engine: Engine, source: &str) -> Runtime {
    let initial = hot_reload_initial_from_source(&engine, source);
    Runtime::from_hot_reload_version(engine, initial)
}

fn hot_reload_initial_from_source(
    engine: &Engine,
    source: &str,
) -> vela_hot_reload::version::ProgramVersion {
    engine
        .compile_hot_reload_initial(SourceId::new(1), source)
        .expect("initial hot reload source compile")
}

fn stage_source_update(runtime: &mut Runtime, source: &str) {
    let update = runtime
        .compile_hot_reload_update(SourceId::new(2), source)
        .expect("runtime should be hot-reload enabled");
    runtime
        .stage_hot_update_result(update)
        .expect("source update should stage");
}

fn write_reward_modules(
    root: &std::path::Path,
    main_return: &str,
    reward: i64,
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
    write_reward_module(&reward_file, reward);
    reward_file
}

fn write_reward_module(path: &std::path::Path, reward: i64) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write reward module");
}

fn write_native_reward_module(path: &std::path::Path, native_name: &str, suffix: &str) {
    std::fs::write(
        path,
        format!(
            r#"
pub fn grant() {{
    return game::native::{native_name}(){suffix};
}}
"#
        ),
    )
    .expect("write native reward module");
}

fn write_schema_reward_modules(
    root: &std::path::Path,
    reward: i64,
    count_field: StructCountField,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_schema_reward_module(&reward_file, reward, count_field);
    reward_file
}

#[derive(Clone, Copy)]
enum StructCountField {
    Absent,
    Defaulted,
    Required,
    Float,
}

impl StructCountField {
    const fn source(self) -> &'static str {
        match self {
            Self::Absent => "",
            Self::Defaulted => "    count: i64 = 1\n",
            Self::Required => "    count: i64\n",
            Self::Float => "    count: f64\n",
        }
    }
}

fn write_schema_reward_module(path: &std::path::Path, reward: i64, count_field: StructCountField) {
    let count_field = count_field.source();
    std::fs::write(
        path,
        format!(
            r#"
struct Reward {{
    item_id: string
{count_field}}}

pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write schema reward module");
}

fn write_stable_schema_rename_modules(
    root: &std::path::Path,
    reward: i64,
    renamed: bool,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_stable_schema_rename_module(&reward_file, reward, renamed);
    reward_file
}

fn write_stable_schema_rename_module(path: &std::path::Path, reward: i64, renamed: bool) {
    let (item_field, count_field, active_variant, finished_variant) = if renamed {
        (
            "item",
            "quantity",
            "Started",
            "    #[id(202)]\n    Finished\n",
        )
    } else {
        ("item_id", "count", "Active", "")
    };
    std::fs::write(
        path,
        format!(
            r#"
struct Reward {{
    #[id(101)]
    {item_field}: string
    #[id(102)]
    {count_field}: i64
}}

enum QuestProgress {{
    #[id(201)]
    {active_variant}
{finished_variant}}}

pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write stable schema rename module");
}

fn write_enum_reward_modules(
    root: &std::path::Path,
    reward: i64,
    count_field: EnumVariantCountField,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_enum_reward_module(&reward_file, reward, count_field);
    reward_file
}

#[derive(Clone, Copy)]
enum EnumVariantCountField {
    Absent,
    Defaulted,
    Required,
    Float,
}

impl EnumVariantCountField {
    const fn source(self) -> &'static str {
        match self {
            Self::Absent => "",
            Self::Defaulted => "        count: i64 = 0\n",
            Self::Required => "        count: i64\n",
            Self::Float => "        count: f64\n",
        }
    }
}

fn write_enum_reward_module(
    path: &std::path::Path,
    reward: i64,
    count_field: EnumVariantCountField,
) {
    let count_field = count_field.source();
    std::fs::write(
        path,
        format!(
            r#"
enum QuestProgress {{
    Active {{
        quest_id: string
{count_field}    }}
}}

pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write enum reward module");
}

fn write_trait_impl_modules(
    root: &std::path::Path,
    reward: i64,
    implemented: bool,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_trait_impl_module(&reward_file, reward, implemented);
    reward_file
}

fn write_trait_impl_module(path: &std::path::Path, reward: i64, implemented: bool) {
    let impl_block = if implemented {
        "impl Damageable for Player {}\n"
    } else {
        ""
    };
    std::fs::write(
        path,
        format!(
            r#"
trait Damageable {{
    fn damage(self) -> i64 {{ return self.level; }}
}}

struct Player {{
    level: i64
}}

{impl_block}
pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write trait impl reward module");
}

fn write_trait_abi_modules(
    root: &std::path::Path,
    reward: i64,
    return_type: &str,
) -> std::path::PathBuf {
    let game_dir = root.join("game");
    std::fs::create_dir_all(&game_dir).expect("create module dir");
    std::fs::write(
        game_dir.join("main.vela"),
        r#"
use game::reward::grant

fn main() {
    return grant();
}
"#,
    )
    .expect("write main module");
    let reward_file = game_dir.join("reward.vela");
    write_trait_abi_module(&reward_file, reward, return_type);
    reward_file
}

fn write_trait_abi_module(path: &std::path::Path, reward: i64, return_type: &str) {
    write_trait_abi_module_with_methods(path, reward, return_type, "");
}

fn write_trait_abi_module_with_required_method(path: &std::path::Path, reward: i64) {
    write_trait_abi_module_with_methods(
        path,
        reward,
        "i64",
        "    fn heal(self, amount: i64) -> i64;\n",
    );
}

fn write_trait_abi_module_with_defaulted_method(path: &std::path::Path, reward: i64) {
    write_trait_abi_module_with_methods(
        path,
        reward,
        "i64",
        "    fn heal(self, amount: i64) -> i64 { return amount; }\n",
    );
}

fn write_trait_abi_module_with_methods(
    path: &std::path::Path,
    reward: i64,
    return_type: &str,
    additional_methods: &str,
) {
    std::fs::write(
        path,
        format!(
            r#"
trait Damageable {{
    fn damage(self, amount: i64) -> {return_type};
{additional_methods}
}}

pub fn grant() {{
    return {reward};
}}
"#
        ),
    )
    .expect("write trait ABI reward module");
}

enum ScriptFunctionReloadWorkflow {
    Directory,
    ChangedFile,
}

