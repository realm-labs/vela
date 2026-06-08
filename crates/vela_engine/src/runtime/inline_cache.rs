use super::image::RuntimeImage;

#[derive(Clone, Debug, Default)]
pub(super) struct InlineCaches {
    _private: (),
}

impl InlineCaches {
    pub(super) fn for_image(_image: &RuntimeImage) -> Self {
        Self::default()
    }

    pub(super) fn clear_for_image(&mut self, image: &RuntimeImage) {
        *self = Self::for_image(image);
    }
}
