use std::alloc::System;
use std::hint::black_box;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use vela_macros::{service, service_set};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[service(path = "test::scalar")]
pub trait ScalarService: Send + Sync {
    fn apply(&self, value: i64) -> i64;
}

pub struct RustScalarService;

impl ScalarService for RustScalarService {
    fn apply(&self, value: i64) -> i64 {
        value.wrapping_mul(3).wrapping_add(1)
    }
}

pub struct RequestContext;

#[service_set(context = RequestContext)]
pub struct TestServices {
    #[vela::default(RustScalarService)]
    pub scalar: dyn ScalarService,
}

#[test]
fn pinned_rust_default_dispatch_allocates_nothing_and_stays_in_rust() {
    let engine = TestServices::register_types(vela_engine::engine::Engine::builder())
        .build()
        .expect("generated scalar registration bundle");
    let services = TestServices::new(&engine.type_bindings()).expect("generated service schema");
    let root = services.pin();
    let mut checksum = 0_i64;
    let region = Region::new(GLOBAL);

    for value in 0..10_000_i64 {
        checksum = checksum.wrapping_add(black_box(root.scalar()).apply(black_box(value)));
    }

    let allocation = region.change();
    assert_eq!(allocation.allocations, 0);
    assert_eq!(allocation.bytes_allocated, 0);
    assert_eq!(checksum, 149_995_000);
}
