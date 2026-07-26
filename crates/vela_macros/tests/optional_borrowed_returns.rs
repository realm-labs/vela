use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use vela_bytecode::UnlinkedInstructionKind;
use vela_engine::engine::Engine;
use vela_engine::interop::{BorrowedReturnOrigin, ReturnMode, ScopedHostAccess};
use vela_engine::native::TypeHint;
use vela_engine::permission::Capability;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_host::error::{HostErrorKind, HostRefLifetimeBoundary};
use vela_macros::{ScriptHost, export, methods};
use vela_vm::error::{VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

#[derive(Debug, ScriptHost)]
#[script(path = "host::Row")]
pub struct Row {
    #[script(get)]
    key: i64,
    #[script(get)]
    value: i64,
}

#[derive(Debug, ScriptHost)]
#[script(path = "host::Table")]
pub struct Table {
    rows: Vec<Row>,
    touches: i64,
    last_returned_address: AtomicUsize,
}

#[derive(Debug, ScriptHost)]
#[script(path = "host::Config")]
pub struct Config {
    table: Table,
}

#[methods(path = "host::Row")]
impl Row {
    pub fn read_value(&self) -> i64 {
        self.value
    }
}

impl Table {
    fn fixture() -> Self {
        Self {
            rows: vec![Row { key: 1, value: 11 }, Row { key: 2, value: 22 }],
            touches: 0,
            last_returned_address: AtomicUsize::new(0),
        }
    }
}

#[methods(path = "host::Config")]
impl Config {
    pub fn table(&self) -> &Table {
        &self.table
    }
}

#[methods(path = "host::Table")]
impl Table {
    #[script_method(reflect = true)]
    pub fn get(&self, key: i64) -> Option<&Row> {
        let row = self.rows.iter().find(|row| row.key == key)?;
        self.last_returned_address
            .store(std::ptr::from_ref(row).addr(), Ordering::SeqCst);
        Some(row)
    }

    pub fn touch(&mut self) -> i64 {
        self.touches += 1;
        self.touches
    }
}

#[export(path = "host::lookup")]
pub fn lookup(table: &Table, key: i64) -> Option<&Row> {
    table.get(key)
}

#[export(path = "host::first_row")]
pub fn first_row(table: &Table) -> &Row {
    &table.rows[0]
}

#[export(path = "host::ready")]
pub async fn ready() -> i64 {
    1
}

fn engine() -> Engine {
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .capability(Capability::ReflectionCall)
        .reflection_policy(vela_reflect::permissions::ReflectPolicy::all())
        .register_host_type::<Row>()
        .register_host_type::<Table>()
        .register_host_type::<Config>()
        .register_exports(vela_export_bundle_lookup())
        .register_exports(vela_export_bundle_first_row())
        .register_exports(vela_export_bundle_ready())
        .register_exports(Row::vela_inherent_exports())
        .register_exports(Table::vela_inherent_exports())
        .register_exports(Config::vela_inherent_exports())
        .build()
        .expect("optional borrowed-return fixture should register")
}

#[test]
fn nested_scoped_method_returns_release_child_before_parent() {
    let engine = engine();
    let program = engine
        .compile_source(
            "fn main(config: Config) { \
                 return config.table().get(1)?.value; \
             }",
        )
        .expect("nested borrowed returns should compile");
    let mut runtime =
        Runtime::new(engine, program).expect("nested borrowed runtime should initialize");
    let config = Config {
        table: Table::fixture(),
    };
    let result = runtime
        .call(
            "main",
            CallArgs::new().with_host_ref("config", &config),
            CallOptions::unbounded(),
        )
        .expect("nested borrowed returns should release in dependency order");

    assert_eq!(runtime.value_to_owned(&result), Ok(OwnedValue::i64(11)));
}

fn runtime(source: &str) -> Runtime {
    let engine = engine();
    let program = engine
        .compile_source(source)
        .expect("optional borrowed-return fixture should compile");
    Runtime::new(engine, program).expect("optional borrowed-return runtime should initialize")
}

fn call(runtime: &mut Runtime, entry: &str, table: &mut Table) -> VmResult<OwnedValue> {
    let result = runtime.call(
        entry,
        CallArgs::new().with_host_mut("table", table),
        CallOptions::unbounded(),
    )?;
    runtime.value_to_owned(&result)
}

fn call_async(runtime: &mut Runtime, entry: &str, table: &mut Table) -> VmResult<OwnedValue> {
    let mut future = Box::pin(runtime.call_async(
        entry,
        CallArgs::new().with_host_mut("table", table),
        CallOptions::unbounded(),
    ));
    let mut context = Context::from_waker(Waker::noop());
    let result = loop {
        if let Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut context) {
            break result;
        }
    }?;
    drop(future);
    runtime.value_to_owned(&result)
}

#[test]
fn optional_borrow_contract_preserves_container_type_and_provenance() {
    let function = vela_callable_contract_lookup();
    assert!(matches!(
        function.returns.ty,
        TypeHint::OptionOf(ref item) if matches!(&**item, TypeHint::Host(_))
    ));
    assert!(matches!(
        function.returns.mode,
        ReturnMode::ScopedHost {
            origin: BorrowedReturnOrigin::Parameter(0),
            child_access: ScopedHostAccess::Shared,
            parent_freeze: ScopedHostAccess::Shared,
        }
    ));

    let method = Table::vela_callable_contract_get();
    assert!(method.access.reflect_callable);
    assert!(matches!(
        method.returns.mode,
        ReturnMode::ScopedHost {
            origin: BorrowedReturnOrigin::Receiver,
            child_access: ScopedHostAccess::Shared,
            parent_freeze: ScopedHostAccess::Shared,
        }
    ));
    assert_ne!(
        function.abi_fingerprint(),
        vela_callable_contract_first_row().abi_fingerprint(),
        "Option must remain part of the callable ABI fingerprint"
    );

    let program = engine()
        .compile_source(
            "fn main(table: Table) { \
                 return table.get(99).is_none() && table.touch() == 1; \
             }",
        )
        .expect("optional borrowed return should compile");
    let code = program
        .bytecode()
        .function("main")
        .expect("compiled function should exist");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::ReleaseBorrowLease { .. }
    )));
}

#[test]
fn some_and_none_use_host_identity_without_cloning_rows() {
    let mut runtime = runtime(
        "fn method(table: Table) { let row = table.get(1)?; return row.value; } \
         fn function(table: Table) { let row = host::lookup(table, 2)?; return row.value; } \
         fn direct(table: Table) { let row = host::first_row(table); return row.value; } \
         fn missing(table: Table) { return table.get(99).is_none() && table.touch() == 1; }",
    );
    let mut table = Table::fixture();
    let first_address = std::ptr::from_ref(&table.rows[0]).addr();

    assert_eq!(
        call(&mut runtime, "method", &mut table),
        Ok(OwnedValue::i64(11))
    );
    assert_eq!(
        table.last_returned_address.load(Ordering::SeqCst),
        first_address,
        "the hot path must retain the exact Rust row rather than a clone"
    );
    assert_eq!(
        call(&mut runtime, "function", &mut table),
        Ok(OwnedValue::i64(22))
    );
    assert_eq!(
        call(&mut runtime, "direct", &mut table),
        Ok(OwnedValue::i64(11)),
        "the existing direct borrowed-return path must remain intact"
    );
    assert_eq!(
        call(&mut runtime, "missing", &mut table),
        Ok(OwnedValue::Bool(true)),
        "None must not create a child lease that freezes the owner"
    );
}

#[test]
fn some_children_retain_independent_leases_until_each_release() {
    let mut runtime = runtime(
        "fn partial(table: Table) { \
             let first = table.get(1)?; \
             let second = table.get(2)?; \
             host::release(first); \
             return table.touch() + second.value; \
         } \
         fn after(table: Table) { return table.touch(); }",
    );
    let mut table = Table::fixture();

    let error = call(&mut runtime, "partial", &mut table)
        .expect_err("the second live child must keep the owner frozen");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::HostObjectBusy { .. })
    ));
    assert_eq!(
        call(&mut runtime, "after", &mut table),
        Ok(OwnedValue::i64(1)),
        "root teardown must release every retained child owner lease"
    );
}

#[test]
fn released_optional_child_rejects_use_after_release() {
    let mut runtime = runtime(
        "fn main(table: Table) { \
             let row = table.get(1)?; \
             let alias = row; \
             host::release(row); \
             return alias.value; \
         }",
    );
    let mut table = Table::fixture();

    let error =
        call(&mut runtime, "main", &mut table).expect_err("released aliases must be expired");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::ExpiredBorrowedHostRef { .. })
    ));
}

#[test]
fn released_optional_child_generation_cannot_alias_a_reused_slot() {
    let mut runtime = runtime(
        "fn main(table: Table) { \
             let stale = table.get(1)?; \
             host::release(stale); \
             let fresh = table.get(2)?; \
             let fresh_value = fresh.value; \
             return stale.value + fresh_value; \
         }",
    );
    let mut table = Table::fixture();

    let error = call(&mut runtime, "main", &mut table)
        .expect_err("a stale child generation must not resolve to the replacement slot");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::ExpiredBorrowedHostRef { .. })
    ));
}

#[test]
fn persistent_state_root_return_and_closure_capture_cannot_escape() {
    let mut runtime = runtime(
        "state saved: Closure = || (); \
         fn save(table: Table) { \
             let row = table.get(1)?; \
             saved = || row.value; \
         } \
         fn return_row(table: Table) { return table.get(1); } \
         fn return_closure(table: Table) { \
             let row = table.get(1)?; \
             return || row.value; \
         }",
    );
    let mut table = Table::fixture();

    for (entry, boundary) in [
        ("save", HostRefLifetimeBoundary::PersistentState),
        ("return_row", HostRefLifetimeBoundary::RootReturn),
        ("return_closure", HostRefLifetimeBoundary::RootReturn),
    ] {
        let error = call(&mut runtime, entry, &mut table)
            .expect_err("call-scoped borrowed values must not escape");
        assert!(matches!(
            error.kind(),
            VmErrorKind::Host(HostErrorKind::BorrowedHostRefEscape {
                boundary: actual,
                ..
            }) if actual == boundary
        ));
    }
}

#[test]
fn live_optional_child_cannot_cross_async_suspend() {
    let mut runtime = runtime(
        "async fn rejected(table: Table) { \
             let row = table.get(1)?; \
             host::ready().await; \
             return row.value; \
         } \
         async fn released(table: Table) { \
             let row = table.get(2)?; \
             let value = row.value; \
             host::release(row); \
             host::ready().await; \
             return value; \
         }",
    );
    let mut table = Table::fixture();

    let error = call_async(&mut runtime, "rejected", &mut table)
        .expect_err("a live borrowed child cannot cross suspension");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::BorrowedHostRefEscape {
            boundary: HostRefLifetimeBoundary::AsyncSuspend,
            ..
        })
    ));
    assert_eq!(
        call_async(&mut runtime, "released", &mut table),
        Ok(OwnedValue::i64(22)),
        "a proven-dead child should release before suspension"
    );
}

#[test]
fn dynamic_and_reflection_calls_do_not_bypass_lifetime_or_access_checks() {
    let mut runtime = runtime(
        "fn dynamic_get(table, key) { return table.get(key); } \
         fn dynamic_escape(table: Table) { return dynamic_get(table, 1); } \
         fn reflected(table: Table) { return reflect::call(table, \"get\", 1); }",
    );
    let mut table = Table::fixture();

    let error = call(&mut runtime, "dynamic_escape", &mut table)
        .expect_err("dynamic dispatch must preserve scoped-return escape checks");
    assert!(matches!(
        error.kind(),
        VmErrorKind::Host(HostErrorKind::BorrowedHostRefEscape {
            boundary: HostRefLifetimeBoundary::RootReturn,
            ..
        })
    ));

    let error = call(&mut runtime, "reflected", &mut table)
        .expect_err("reflected borrowed returns must keep root escape checks");
    assert!(
        matches!(
            error.kind(),
            VmErrorKind::Host(HostErrorKind::BorrowedHostRefEscape {
                boundary: HostRefLifetimeBoundary::RootReturn,
                ..
            })
        ),
        "{error:?}"
    );
}
