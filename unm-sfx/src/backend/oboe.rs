use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use oboe::{
    AudioOutputCallback, AudioOutputStreamSafe, AudioStream, AudioStreamAsync, AudioStreamBase,
    AudioStreamBuilder, DataCallbackResult, Error, Output, PerformanceMode, SharingMode, Stereo,
    Usage,
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};

use crate::{
    atlas::{RawSource, SoundAtlas},
    backend::AudioBackend,
    clip::{SfxHandle, MAX_SOUND_COUNT},
    decoder,
    mixer::Mixer,
    player::PLAY_QUEUE_CAPACITY,
};

struct OboeCallback {
    consumer: ringbuf::HeapCons<SfxHandle>,
    mixer: Mixer,
    atlas: SoundAtlas,
    pending_handles: [SfxHandle; PLAY_QUEUE_CAPACITY],
    device_lost: Arc<AtomicBool>,
}

impl OboeCallback {
    fn new(
        consumer: ringbuf::HeapCons<SfxHandle>,
        atlas: SoundAtlas,
        device_lost: Arc<AtomicBool>,
    ) -> Self {
        Self {
            consumer,
            mixer: Mixer::new(),
            atlas,
            pending_handles: [SfxHandle::default(); PLAY_QUEUE_CAPACITY],
            device_lost,
        }
    }
}

impl AudioOutputCallback for OboeCallback {
    type FrameType = (f32, Stereo);

    fn on_audio_ready(
        &mut self,
        _stream: &mut dyn AudioOutputStreamSafe,
        data: &mut [(f32, f32)],
    ) -> DataCallbackResult {
        let data = unsafe {
            std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut f32, data.len() * 2)
        };

        loop {
            let queued = self.consumer.pop_slice(&mut self.pending_handles);
            if queued == 0 {
                break;
            }

            for &handle in &self.pending_handles[..queued] {
                let clip = unsafe { self.atlas.clip_unchecked(handle) };
                let _ = self.mixer.add_sound(clip);
            }
        }

        self.mixer.mix(2, data);
        DataCallbackResult::Continue
    }

    fn on_error_before_close(
        &mut self,
        _audio_stream: &mut dyn AudioOutputStreamSafe,
        _error: Error,
    ) {
        self.device_lost.store(true, Ordering::Release);
    }
}

pub struct Player {
    producer: ringbuf::HeapProd<SfxHandle>,
    consumer: Option<ringbuf::HeapCons<SfxHandle>>,
    stream: Option<AudioStreamAsync<Output, OboeCallback>>,
    device_sample_rate: u32,
    cached_sources: Option<Vec<RawSource>>,
    device_lost: Arc<AtomicBool>,
}

impl Player {
    pub(crate) fn new() -> Self {
        let (producer, consumer) = Self::new_queue();

        Self {
            producer,
            consumer: Some(consumer),
            stream: None,
            device_sample_rate: 48_000,
            cached_sources: None,
            device_lost: Arc::new(AtomicBool::new(false)),
        }
    }

    fn new_queue() -> (ringbuf::HeapProd<SfxHandle>, ringbuf::HeapCons<SfxHandle>) {
        HeapRb::<SfxHandle>::new(PLAY_QUEUE_CAPACITY).split()
    }

    fn reset_queue(&mut self) {
        let (producer, consumer) = Self::new_queue();
        self.producer = producer;
        self.consumer = Some(consumer);
    }

    fn loaded_sound_count(&self) -> usize {
        self.cached_sources.as_ref().map_or(0, Vec::len)
    }

    fn drop_stream(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.stop();
        }
    }

    fn load_sources(&mut self, sources: Vec<RawSource>) -> Option<Vec<SfxHandle>> {
        if sources.len() > MAX_SOUND_COUNT {
            return None;
        }

        let handles = (0..sources.len())
            .map(SfxHandle::from_index)
            .collect::<Vec<_>>();

        self.cached_sources = Some(sources);
        self.build_stream().ok()?;
        Some(handles)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.drop_stream();
    }
}

impl AudioBackend for Player {
    fn maintain_stream(&mut self) {
        if self.device_lost.load(Ordering::Acquire) {
            self.drop_stream();
            self.reset_queue();
            self.device_lost.store(false, Ordering::Release);
        }

        if self.cached_sources.is_some() && self.stream.is_none() {
            let _ = self.build_stream();
        }
    }

    fn shutdown(&mut self) {
        self.drop_stream();
        self.reset_queue();
        self.cached_sources = None;
        self.device_lost.store(false, Ordering::Release);
    }

    fn build_stream(&mut self) -> anyhow::Result<()> {
        if self.cached_sources.is_none() {
            return Ok(());
        }

        self.drop_stream();
        self.reset_queue();

        let probe_stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Exclusive)
            .set_usage(Usage::Game)
            .set_channel_count::<Stereo>()
            .set_format::<f32>()
            .open_stream()?;
        self.device_sample_rate = probe_stream.get_sample_rate() as u32;
        drop(probe_stream);

        let consumer = self
            .consumer
            .take()
            .ok_or_else(|| anyhow::anyhow!("Missing consumer queue"))?;
        let atlas = {
            let sources = self
                .cached_sources
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing cached sources"))?;
            SoundAtlas::build_from_sources(sources, self.device_sample_rate)
        };
        let callback = OboeCallback::new(
            consumer,
            atlas,
            Arc::clone(&self.device_lost),
        );

        self.device_lost.store(false, Ordering::Release);

        let mut stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Exclusive)
            .set_usage(Usage::Game)
            .set_channel_count::<Stereo>()
            .set_format::<f32>()
            .set_callback(callback)
            .open_stream()?;

        stream.start()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn init_load_sound(&mut self, datas: Vec<Vec<u8>>) -> Option<Vec<SfxHandle>> {
        if datas.len() > MAX_SOUND_COUNT {
            return None;
        }

        let mut sounds = Vec::with_capacity(datas.len());
        for data in datas {
            sounds.push(decoder::decode(data).ok()?);
        }

        self.load_sources(sounds)
    }

    fn init_load_sound_from_sources(&mut self, sources: Vec<RawSource>) -> Option<Vec<SfxHandle>> {
        self.load_sources(sources)
    }

    fn play(&mut self, handle: SfxHandle) {
        if !handle.is_valid_for_len(self.loaded_sound_count()) {
            return;
        }

        let _ = self.producer.try_push(handle);
    }
}
