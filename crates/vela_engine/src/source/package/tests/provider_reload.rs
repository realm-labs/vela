use super::*;

#[test]
fn provider_body_change_is_accepted() {
    provider_handle_rebinds_after_compatible_reload();
}

#[test]
fn provider_target_or_signature_change_is_rejected() {
    let root = package_fixture("provider_target_reload");
    let source = |target: &str| {
        format!(
            r#"pub trait CommandProvider {{ fn run(self) -> i64; }}
pub struct First {{}}
pub struct Second {{}}
#[provider(id = "command")]
impl CommandProvider for {target} {{ pub fn run(self) -> i64 {{ return 1; }} }}
"#
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source("First"));
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let first_catalog = engine.discover_providers(&first).expect("first catalog");
    let key = first_catalog.providers()[0].key().clone();
    let first_selection = first_catalog.select([key.clone()]).expect("selection");
    let first_request = ProviderCompileRequest::for_selection(&first, first_selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &first_request)
        .expect("initial version");

    fs::write(root.join("src/api.vela"), source("Second")).expect("change provider target");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let second_catalog = engine.discover_providers(&second).expect("second catalog");
    let second_selection = second_catalog.select([key]).expect("selection");
    let second_request = ProviderCompileRequest::for_selection(&second, second_selection);
    let error = engine
        .compile_provider_hot_reload_update(&initial, &second, &second_request)
        .expect_err("provider target change rejected");

    assert!(matches!(
        &error.kind,
        EnginePackageErrorKind::HotReload(vela_hot_reload::error::HotReloadError {
            kind: vela_hot_reload::error::HotReloadErrorKind::ChangedPackageProviderAbi { .. }
        })
    ));
    let EnginePackageErrorKind::HotReload(error) = &error.kind else {
        unreachable!("matched hot reload error")
    };
    assert!(error.source_span().is_some());
    assert_eq!(
        error.manifest_path(),
        Some(
            fs::canonicalize(root.join("vela.toml"))
                .expect("canonical manifest")
                .as_path()
        )
    );
    remove_fixture(root);
}

#[test]
fn service_trait_method_change_is_rejected() {
    let root = package_fixture("provider_service_reload");
    let source = |parameter: &str, argument: &str| {
        format!(
            r#"pub trait CommandProvider {{ fn run(self{parameter}) -> i64; }}
pub struct Command {{}}
#[provider(id = "command")]
impl CommandProvider for Command {{ pub fn run(self{parameter}) -> i64 {{ return 1{argument}; }} }}
"#
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source("", ""));
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let catalog = engine.discover_providers(&first).expect("catalog");
    let key = catalog.providers()[0].key().clone();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&first, selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(
        root.join("src/api.vela"),
        source(", value: i64", " + value"),
    )
    .expect("change service signature");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let catalog = engine.discover_providers(&second).expect("catalog");
    let selection = catalog.select([key]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&second, selection);

    assert!(
        engine
            .compile_provider_hot_reload_update(&initial, &second, &request)
            .is_err()
    );
    remove_fixture(root);
}

#[test]
fn capability_expansion_requires_host_approval() {
    let root = package_fixture("provider_capability_reload");
    let source = |body: &str| {
        format!(
            r#"pub trait ClockProvider {{ fn now(self) -> i64; }}
pub struct Clock {{}}
#[provider(id = "clock")]
impl ClockProvider for Clock {{ pub fn now(self) -> i64 {{ {body} }} }}
"#
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source("return 1;"));
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .with_time_clock(1, 1)
        .build()
        .expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let first_catalog = engine.discover_providers(&first).expect("first catalog");
    let key = first_catalog.providers()[0].key().clone();
    let first_selection = first_catalog.select([key.clone()]).expect("selection");
    let first_request = ProviderCompileRequest::for_selection(&first, first_selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &first_request)
        .expect("initial version");

    fs::write(
        root.join("vela.toml"),
        "[package]\nid = \"dev.vela.plugin\"\nname = \"plugin\"\nversion = \"0.1.0\"\n[source]\nroots = [\"src\"]\n[capabilities]\nrequires = [\"time\"]\n",
    )
    .expect("expand manifest capabilities");
    fs::write(root.join("src/api.vela"), source("return time::now();"))
        .expect("use expanded capability");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let second_catalog = engine.discover_providers(&second).expect("second catalog");
    let second_selection = second_catalog.select([key]).expect("selection");
    let second_request = ProviderCompileRequest::for_selection(&second, second_selection);
    let error = engine
        .compile_provider_hot_reload_update(&initial, &second, &second_request)
        .expect_err("capability expansion needs explicit restaging");

    assert!(matches!(
        error.kind,
        EnginePackageErrorKind::HotReload(vela_hot_reload::error::HotReloadError {
            kind: vela_hot_reload::error::HotReloadErrorKind::ChangedPackageProviderAbi { .. }
        })
    ));
    remove_fixture(root);
}

#[test]
fn provider_removal_is_rejected_without_advancing_active_image() {
    let root = package_fixture("provider_removal_reload");
    let source = r#"pub trait CommandProvider { fn run(self) -> i64; }
pub struct Command {}
#[provider(id = "command")]
impl CommandProvider for Command { pub fn run(self) -> i64 { return 1; } }
"#;
    write_package(&root, "dev.vela.plugin", "plugin", "", source);
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let catalog = engine.discover_providers(&first).expect("catalog");
    let key = catalog.providers()[0].key().clone();
    let method = catalog.providers()[0].methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&first, selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &request)
        .expect("initial version");
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone());
    let handle = runtime.provider_handle(&key).expect("provider handle");

    fs::write(
        root.join("src/api.vela"),
        "pub trait CommandProvider { fn run(self) -> i64; }\npub struct Command {}\n",
    )
    .expect("remove selected provider");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    assert!(
        engine
            .compile_package_hot_reload_update_from_previous(&initial, &second)
            .is_err()
    );
    assert_eq!(call_provider_i64(&mut runtime, &handle, method), 1);
    remove_fixture(root);
}

#[test]
fn unselected_provider_addition_does_not_change_runtime_abi() {
    let root = package_fixture("provider_unselected_addition");
    let source = |extra: &str| {
        format!(
            r#"pub trait CommandProvider {{ fn run(self) -> i64; }}
pub struct Selected {{}}
#[provider(id = "selected")]
impl CommandProvider for Selected {{ pub fn run(self) -> i64 {{ return 1; }} }}
{extra}
"#
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source(""));
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let catalog = engine.discover_providers(&first).expect("catalog");
    let key = catalog.providers()[0].key().clone();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&first, selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(
        root.join("src/api.vela"),
        source(
            r#"pub struct Added {}
#[provider(id = "added")]
impl CommandProvider for Added { pub fn run(self) -> i64 { return 2; } }"#,
        ),
    )
    .expect("add unselected provider");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let update = engine
        .compile_package_hot_reload_update_from_previous(&initial, &second)
        .expect("unselected addition accepted");
    let installed = update
        .linked_artifact()
        .package_metadata()
        .expect("package metadata")
        .installed_providers();
    assert_eq!(installed.len(), 1);
    assert!(installed.get(&key).is_some());
    remove_fixture(root);
}

#[test]
fn ordinary_reload_reapplies_previous_provider_selection() {
    unselected_provider_addition_does_not_change_runtime_abi();
}

#[test]
fn old_frame_keeps_old_provider_generation_and_new_call_uses_new_generation() {
    let root = package_fixture("provider_retained_closure_reload");
    let source = |value| {
        format!(
            r#"fn helper() {{ return {value}; }}
pub fn invoke(callback) {{ return callback(); }}
pub trait CallbackProvider {{ fn callback(self); }}
pub struct Callback {{}}
#[provider(id = "callback")]
impl CallbackProvider for Callback {{
    pub fn callback(self) {{ return || helper(); }}
}}
"#
        )
    };
    write_package(&root, "dev.vela.plugin", "plugin", "", &source(1));
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let catalog = engine.discover_providers(&first).expect("catalog");
    let key = catalog.providers()[0].key().clone();
    let method = catalog.providers()[0].methods()[0].id();
    let selection = catalog.select([key.clone()]).expect("selection");
    let request = ProviderCompileRequest::for_selection(&first, selection);
    let initial = engine
        .compile_provider_hot_reload_initial(&first, &request)
        .expect("initial version");
    let mut runtime = Runtime::from_hot_reload_version(engine.clone(), initial.clone());
    let handle = runtime.provider_handle(&key).expect("provider handle");
    let old_callback = runtime
        .call_provider_handle(&handle, method, CallArgs::new(), CallOptions::unbounded())
        .expect("old callback");

    fs::write(root.join("src/api.vela"), source(2)).expect("change provider helper body");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let update = engine
        .compile_package_hot_reload_update_from_previous(&initial, &second)
        .expect("compatible provider update");
    runtime.stage_hot_update(update).expect("stage update");
    runtime
        .check_reload()
        .expect("safe point")
        .expect("accepted report");
    let new_callback = runtime
        .call_provider_handle(&handle, method, CallArgs::new(), CallOptions::unbounded())
        .expect("new callback");

    assert_eq!(call_callback_i64(&mut runtime, old_callback), 1);
    assert_eq!(call_callback_i64(&mut runtime, new_callback), 2);
    remove_fixture(root);
}

#[test]
fn ordinary_package_reload_reapplies_previous_root_set() {
    let root = package_fixture("ordinary_root_reapply");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "",
        "pub fn main() { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(root.join("src/main.vela"), "pub fn main() { return 2; }\n").expect("change body");
    let update = engine
        .compile_package_workspace_hot_reload_update_from_previous(&initial, root.join("vela.toml"))
        .expect("previous roots reapplied");

    assert_eq!(
        update
            .linked_artifact()
            .package_metadata()
            .expect("package metadata")
            .request()
            .roots(),
        &[package("dev.vela.app")]
    );
    remove_fixture(root);
}

#[test]
fn dependency_change_reports_impacted_packages() {
    let root = package_fixture("package_impact_report");
    write_package(
        &root,
        "dev.vela.app",
        "app",
        "[dependencies]\nutil = { path = \"util\" }\n",
        "use util::api::answer\npub fn main() { return answer(); }\n",
    );
    write_package(
        &root.join("util"),
        "dev.vela.util",
        "util",
        "",
        "pub fn answer() { return 1; }\n",
    );
    let engine = Engine::builder().build().expect("engine");
    let first = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("first snapshot");
    let request = PackageCompileRequest::for_root(&first, &package("dev.vela.app"));
    let initial = engine
        .compile_package_hot_reload_initial(&first, &request)
        .expect("initial version");

    fs::write(
        root.join("util/src/api.vela"),
        "pub fn answer() { return 2; }\n",
    )
    .expect("change dependency body");
    let second = engine
        .load_package_workspace(root.join("vela.toml"))
        .expect("second snapshot");
    let update = engine
        .compile_package_hot_reload_update_from_previous(&initial, &second)
        .expect("dependency body update");

    assert_eq!(update.changed_packages(), &["dev.vela.util"]);
    assert!(
        update
            .impacted_packages()
            .contains(&"dev.vela.app".to_owned())
    );
    remove_fixture(root);
}

fn call_callback_i64(runtime: &mut Runtime, callback: crate::runtime::VelaValue) -> i64 {
    let output = runtime
        .call(
            "api::invoke",
            CallArgs::new().with_named_vela_value("callback", callback),
            CallOptions::unbounded(),
        )
        .expect("invoke callback");
    match runtime
        .value_to_owned(&output)
        .expect("materialize callback")
    {
        OwnedValue::Scalar(ScalarValue::I64(value)) => value,
        other => panic!("expected callback i64, got {other:?}"),
    }
}
