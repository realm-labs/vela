use std::ops::Deref;
use std::sync::Arc;

use vela_bytecode::linker::LinkError;
use vela_bytecode::{LinkedArtifact, LinkedProgram, ProgramImage};
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::ProgramVersion;

use crate::engine::Engine;

pub struct RuntimeImage {
    engine: Engine,
    artifact: Arc<LinkedArtifact>,
    version_id: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
}

pub struct OwnedImage {
    image: RuntimeImage,
}

#[derive(Clone)]
pub struct SharedImage {
    image: Arc<RuntimeImage>,
}

pub trait RuntimeImageStorage: Deref<Target = RuntimeImage> {
    #[doc(hidden)]
    fn from_runtime_image(image: RuntimeImage) -> Self;
}

pub(super) struct RuntimeImageLayout {
    global_names: Box<[String]>,
}

impl OwnedImage {
    #[must_use]
    pub fn from_image(image: RuntimeImage) -> Self {
        Self { image }
    }
}

impl SharedImage {
    #[must_use]
    pub fn from_arc(image: Arc<RuntimeImage>) -> Self {
        Self { image }
    }
}

impl Deref for OwnedImage {
    type Target = RuntimeImage;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

impl Deref for SharedImage {
    type Target = RuntimeImage;

    fn deref(&self) -> &Self::Target {
        self.image.as_ref()
    }
}

impl RuntimeImageStorage for OwnedImage {
    fn from_runtime_image(image: RuntimeImage) -> Self {
        Self::from_image(image)
    }
}

impl RuntimeImageStorage for SharedImage {
    fn from_runtime_image(image: RuntimeImage) -> Self {
        image.into_shared()
    }
}

impl RuntimeImage {
    #[must_use]
    pub fn new_compiled(engine: Engine, program: vela_bytecode::compiler::CompiledProgram) -> Self {
        Self::try_new_compiled(engine, program)
            .expect("compiled runtime image should link verified bytecode")
    }

    pub fn try_new_compiled(
        engine: Engine,
        program: vela_bytecode::compiler::CompiledProgram,
    ) -> Result<Self, LinkError> {
        let artifact = engine.link_compiled_program(program)?;
        let layout = RuntimeImageLayout::from_global_names(artifact.image().global_names());
        Ok(Self {
            engine,
            artifact,
            version_id: None,
            layout,
        })
    }

    #[must_use]
    pub fn from_program_version(engine: Engine, version: &ProgramVersion) -> Self {
        let version_id = Some(version.id);
        let artifact = Arc::clone(version.linked_artifact());
        let layout = RuntimeImageLayout::from_global_names(artifact.image().global_names());
        Self {
            engine,
            artifact,
            version_id,
            layout,
        }
    }

    pub(super) const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(super) fn program_image(&self) -> &ProgramImage {
        self.artifact.image()
    }

    pub fn linked_program(&self) -> &LinkedProgram {
        self.artifact.program()
    }

    pub(super) fn linked_artifact(&self) -> &Arc<LinkedArtifact> {
        &self.artifact
    }

    pub(super) fn global_names(&self) -> &[String] {
        self.layout.global_names()
    }

    #[cfg(test)]
    pub(super) fn cache_site_count(&self) -> usize {
        self.artifact.cache_layout().len()
    }

    pub(super) fn current_program_version_id(&self) -> Option<ProgramVersionId> {
        self.version_id
    }

    #[must_use]
    pub fn into_shared(self) -> SharedImage {
        SharedImage::from_arc(Arc::new(self))
    }
}

impl RuntimeImageLayout {
    fn from_global_names(names: &[String]) -> Self {
        Self {
            global_names: names.to_vec().into_boxed_slice(),
        }
    }

    fn global_names(&self) -> &[String] {
        &self.global_names
    }
}

#[cfg(test)]
mod tests {
    use vela_bytecode::linked::InstructionKind;
    use vela_def::FunctionId;
    use vela_vm::owned_value::OwnedValue;

    use crate::engine::Engine;
    use crate::native::{NativeFunctionDesc, NativeFunctionId};

    use super::RuntimeImage;

    #[test]
    fn runtime_image_builds_indexed_program_sidecar() {
        let engine = Engine::builder().build().expect("engine should build");
        let program = engine
            .compile_source(
                "global state: i64; fn main() { return state; } fn helper() { return state; }",
            )
            .expect("fixture compiles");
        let image = RuntimeImage::new_compiled(engine, program);

        assert_eq!(image.global_names(), &["main::state".to_owned()]);
        assert_eq!(image.cache_site_count(), 2);
        assert_eq!(image.linked_program().function_count(), 2);
        let main_index = image
            .program_image()
            .function_index("main")
            .expect("main function should have image index");
        assert_eq!(
            image
                .program_image()
                .function(main_index)
                .expect("main index should resolve")
                .name,
            "main"
        );
    }

    #[test]
    fn runtime_image_uses_linker_owned_record_cache_site_operands() {
        let engine = Engine::builder().build().expect("engine should build");
        let program = engine
            .compile_source(
                "struct Item { score: i64 } fn first(value: Item) { return value.score; } fn second(value: Item) { value.score = value.score + 1; return value.score; }",
            )
            .expect("fixture compiles");
        let image = RuntimeImage::new_compiled(engine, program);
        let linked = image.linked_program();
        let first_site = record_read_site(linked, "first");
        let second_site = record_read_site(linked, "second");
        let second_write_site = record_write_site(linked, "second");

        assert!(image.cache_site_count() >= 3);
        assert_eq!(first_site, Some(vela_bytecode::CacheSiteId::new(0)));
        assert_eq!(second_site, Some(vela_bytecode::CacheSiteId::new(1)));
        assert_eq!(second_write_site, Some(vela_bytecode::CacheSiteId::new(2)));
    }

    #[test]
    fn runtime_image_links_with_engine_native_implementations() {
        let native_id = NativeFunctionId::new(91);
        let engine = Engine::builder()
            .register_native_fn(NativeFunctionDesc::new("test::answer", native_id), |_| {
                Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(42)))
            })
            .build()
            .expect("engine should build");
        let program = engine
            .compile_source("fn main() { return test::answer(); }")
            .expect("fixture compiles");
        let image = RuntimeImage::new_compiled(engine, program);

        let linked = image.linked_program();
        assert_eq!(linked.function_count(), 1);
        assert_eq!(linked.native_function_count(), 1);
        let linked_native = image
            .linked_program()
            .native_functions()
            .next()
            .map(|(_, native)| native.id);
        assert_eq!(linked_native, Some(FunctionId::new(91)));
    }

    fn record_read_site(
        linked: &vela_bytecode::LinkedProgram,
        function_name: &str,
    ) -> Option<vela_bytecode::CacheSiteId> {
        let code = linked
            .functions()
            .find(|(_, code)| linked.debug_name(code.debug_name) == function_name)
            .map(|(_, code)| code)?;
        code.instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::GetRecordSlot { cache_site, .. } => cache_site,
                _ => None,
            })
    }

    fn record_write_site(
        linked: &vela_bytecode::LinkedProgram,
        function_name: &str,
    ) -> Option<vela_bytecode::CacheSiteId> {
        let code = linked
            .functions()
            .find(|(_, code)| linked.debug_name(code.debug_name) == function_name)
            .map(|(_, code)| code)?;
        code.instructions
            .iter()
            .find_map(|instruction| match instruction.kind {
                InstructionKind::SetRecordSlot { cache_site, .. } => cache_site,
                _ => None,
            })
    }
}
