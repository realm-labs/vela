use std::cell::RefCell;

use vela_bytecode::CacheSiteId;
use vela_common::GlobalSlot;

use super::image::RuntimeImage;

#[derive(Debug, Default)]
pub(super) struct InlineCaches {
    entries: RefCell<Vec<InlineCacheEntry>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InlineCacheEntry {
    Empty,
    GlobalRead { slot: GlobalSlot },
}

impl InlineCaches {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        Self {
            entries: RefCell::new(vec![InlineCacheEntry::Empty; image.cache_site_count()]),
        }
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }

    pub(super) fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    pub(super) fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        match self.entries.borrow().get(site.index()) {
            Some(InlineCacheEntry::GlobalRead { slot }) => Some(*slot),
            _ => None,
        }
    }

    pub(super) fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        if let Some(entry) = self.entries.borrow_mut().get_mut(site.index()) {
            *entry = InlineCacheEntry::GlobalRead { slot };
        }
    }
}

impl vela_vm::VmInlineCaches for InlineCaches {
    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn global_read_slot(&self, site: CacheSiteId) -> Option<GlobalSlot> {
        self.global_read_slot(site)
    }

    fn set_global_read_slot(&self, site: CacheSiteId, slot: GlobalSlot) {
        self.set_global_read_slot(site, slot);
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::CacheSiteKind;
    use vela_bytecode::compiler::compile_program_source_with_options;
    use vela_common::SourceId;
    use vela_vm::owned_value::OwnedValue;

    use crate::engine::Engine;
    use crate::runtime::{CallArgs, CallOptions, Runtime, RuntimeImage};

    use super::InlineCaches;

    #[test]
    fn inline_caches_allocate_from_image_cache_site_count() {
        let engine = Engine::builder().build().expect("engine should build");
        let cached_program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
global value: Int;

fn main() {
    return value;
}
"#,
            &engine.compiler_options(),
        )
        .expect("program should compile");
        let cached_image = RuntimeImage::new(engine.clone(), cached_program);
        let mut caches = InlineCaches::for_image(&cached_image);

        assert!(cached_image.cache_site_count() > 0);
        assert!(!caches.is_empty());
        assert_eq!(caches.len(), cached_image.cache_site_count());

        let empty_program = compile_program_source_with_options(
            SourceId::new(2),
            "fn main() { return 1; }",
            &engine.compiler_options(),
        )
        .expect("program should compile");
        let empty_image = RuntimeImage::new(engine, empty_program);
        caches.clear_for_image(&empty_image);

        assert_eq!(empty_image.cache_site_count(), 0);
        assert!(caches.is_empty());
        assert_eq!(caches.len(), 0);
    }

    #[test]
    fn global_read_inline_cache_is_runtime_local_and_site_indexed() {
        let engine = Engine::builder().build().expect("engine should build");
        let program = compile_program_source_with_options(
            SourceId::new(1),
            r#"
global first: Int;
global second: Int;

fn read_first() {
    return first;
}

fn read_second() {
    return second;
}
"#,
            &engine.compiler_options(),
        )
        .expect("program should compile");
        let first_slot = program
            .global_slot("main::first")
            .expect("first global should have slot");
        let second_slot = program
            .global_slot("main::second")
            .expect("second global should have slot");

        let mut runtime = Runtime::new(engine, program);
        let first_site = runtime
            .image
            .program_image()
            .function_by_name("read_first")
            .expect("read_first should exist")
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::GlobalRead)
            .expect("read_first should have global read site")
            .id;
        let second_site = runtime
            .image
            .program_image()
            .function_by_name("read_second")
            .expect("read_second should exist")
            .cache_sites
            .sites()
            .iter()
            .find(|site| site.kind == CacheSiteKind::GlobalRead)
            .expect("read_second should have global read site")
            .id;
        assert_ne!(first_site, second_site);
        runtime
            .insert_global("main::first", OwnedValue::Int(10))
            .expect("first global should insert");
        runtime
            .insert_global("main::second", OwnedValue::Int(20))
            .expect("second global should insert");

        assert_eq!(
            runtime.state.inline_caches.global_read_slot(first_site),
            None
        );

        let first = runtime
            .call("read_first", CallArgs::new(), CallOptions::unbounded())
            .expect("read_first should run");
        assert_eq!(runtime.value_to_owned(&first), Ok(OwnedValue::Int(10)));
        assert_eq!(
            runtime.state.inline_caches.global_read_slot(first_site),
            Some(first_slot)
        );

        let second = runtime
            .call("read_second", CallArgs::new(), CallOptions::unbounded())
            .expect("read_second should run");
        assert_eq!(runtime.value_to_owned(&second), Ok(OwnedValue::Int(20)));
        assert_eq!(
            runtime.state.inline_caches.global_read_slot(second_site),
            Some(second_slot)
        );
        assert_eq!(
            runtime.state.inline_caches.global_read_slot(first_site),
            Some(first_slot)
        );

        runtime
            .insert_global("main::first", OwnedValue::Int(30))
            .expect("first global should update");
        let first_after_update = runtime
            .call("read_first", CallArgs::new(), CallOptions::unbounded())
            .expect("read_first should run after update");
        assert_eq!(
            runtime.value_to_owned(&first_after_update),
            Ok(OwnedValue::Int(30))
        );
    }
}
