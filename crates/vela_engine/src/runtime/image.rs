use std::ops::Deref;
use std::sync::Arc;

use vela_bytecode::{ProgramImage, UnlinkedProgram};
use vela_hot_reload::profile::ProgramProfile;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_hot_reload::version::ProgramVersion;

use crate::engine::Engine;

pub struct RuntimeImage {
    engine: Engine,
    program_image: ProgramImage,
    version_id: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    #[allow(dead_code)]
    profile: Option<ProgramProfile>,
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
    pub fn new(engine: Engine, program: UnlinkedProgram) -> Self {
        let program_image = ProgramImage::from_program(&program);
        let layout = RuntimeImageLayout::from_global_names(program_image.global_names());
        Self {
            engine,
            program_image,
            version_id: None,
            layout,
            profile: None,
        }
    }

    #[must_use]
    pub fn from_program_version(engine: Engine, version: &ProgramVersion) -> Self {
        let version_id = Some(version.id);
        let profile = Some(version.profile().clone());
        let program_image = version.program_image().clone();
        let layout = RuntimeImageLayout::from_global_names(program_image.global_names());
        Self {
            engine,
            program_image,
            version_id,
            layout,
            profile,
        }
    }

    pub(super) const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(super) const fn program_image(&self) -> &ProgramImage {
        &self.program_image
    }

    pub(super) fn global_names(&self) -> &[String] {
        self.layout.global_names()
    }

    pub(super) fn cache_site_count(&self) -> usize {
        self.program_image.cache_site_count()
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
    use vela_bytecode::{CacheSiteKind, InstructionOffset, UnlinkedCodeObject, UnlinkedProgram};

    use crate::engine::Engine;

    use super::RuntimeImage;

    #[test]
    fn runtime_image_builds_indexed_program_sidecar() {
        let mut main = UnlinkedCodeObject::new("main", 0);
        main.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
        let mut helper = UnlinkedCodeObject::new("helper", 0);
        helper.push_cache_site(CacheSiteKind::NativeCall, InstructionOffset(0));

        let mut program = UnlinkedProgram::new();
        program.set_global_layout(["main::state".to_owned()]);
        program.insert_function(main);
        program.insert_function(helper);

        let engine = Engine::builder().build().expect("engine should build");
        let image = RuntimeImage::new(engine, program);

        assert_eq!(image.global_names(), &["main::state".to_owned()]);
        assert_eq!(image.cache_site_count(), 2);
        let main_index = image
            .program_image
            .function_index("main")
            .expect("main function should have image index");
        assert_eq!(
            image
                .program_image
                .function(main_index)
                .expect("main index should resolve")
                .name,
            "main"
        );
    }
}
