#[cfg(any(target_os = "android"))]
pub mod oboe;

#[cfg(not(target_os = "android"))]
pub mod cpal;

use crate::atlas::RawSource;
use crate::clip::SfxHandle;

pub trait AudioBackend {
    fn build_stream(&mut self) -> anyhow::Result<()>;

    fn maintain_stream(&mut self);

    fn shutdown(&mut self);

    fn init_load_sound(&mut self, datas: Vec<Vec<u8>>) -> Option<Vec<SfxHandle>>;

    fn init_load_sound_from_sources(&mut self, sources: Vec<RawSource>) -> Option<Vec<SfxHandle>>;

    fn play(&mut self, handle: SfxHandle);

    fn submit_frame_play_count(&mut self, handle: SfxHandle, count: u16);
}
