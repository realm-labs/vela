use super::image::RuntimeImage;

#[derive(Clone, Debug, Default)]
pub(super) struct InlineCaches {
    entries: Vec<InlineCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InlineCacheEntry {
    Empty,
}

impl InlineCaches {
    pub(super) fn for_image(image: &RuntimeImage) -> Self {
        Self {
            entries: vec![InlineCacheEntry::Empty; image.cache_site_count()],
        }
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl vela_vm::VmInlineCaches for InlineCaches {
    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_program_source_with_options;
    use vela_common::SourceId;

    use crate::engine::Engine;
    use crate::runtime::RuntimeImage;

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
}
