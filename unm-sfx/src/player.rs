use crate::{
    atlas::RawSource,
    backend::AudioBackend,
    clip::{SfxHandle, MAX_SOUND_COUNT},
};

pub(crate) const COMMAND_QUEUE_CAPACITY: usize = 256;

#[derive(Clone, Copy, Default)]
pub(crate) enum PlayCommand {
    #[default]
    None,
    Single(SfxHandle),
    Repeat { handle: SfxHandle, count: u16 },
}

pub struct SfxManager(Box<dyn AudioBackend>);

unsafe impl Send for SfxManager {}
unsafe impl Sync for SfxManager {}

impl SfxManager {
    pub fn new() -> Self {
        #[cfg(target_os = "android")]
        let backend = Box::new(crate::backend::oboe::Player::new());
        #[cfg(not(target_os = "android"))]
        let backend = Box::new(crate::backend::cpal::Player::new());

        Self(backend)
    }

    pub fn maintain_stream(&mut self) {
        self.0.maintain_stream()
    }

    pub fn shutdown(&mut self) {
        self.0.shutdown()
    }

    pub fn init_load_sound(&mut self, datas: Vec<Vec<u8>>) -> Option<Vec<SfxHandle>> {
        if datas.len() > MAX_SOUND_COUNT {
            return None;
        }

        self.0.init_load_sound(datas)
    }

    pub fn init_load_sound_from_sources(
        &mut self,
        sources: Vec<RawSource>,
    ) -> Option<Vec<SfxHandle>> {
        if sources.len() > MAX_SOUND_COUNT {
            return None;
        }

        self.0.init_load_sound_from_sources(sources)
    }

    pub fn play(&mut self, handle: SfxHandle) {
        self.0.play(handle);
    }

    pub fn submit_frame_play_count(&mut self, handle: SfxHandle, count: u16) {
        self.0.submit_frame_play_count(handle, count);
    }
}
