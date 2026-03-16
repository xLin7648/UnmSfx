use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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

struct CallbackState {
    consumer: ringbuf::HeapCons<SfxHandle>,
    mixer: Mixer,
    atlas: SoundAtlas,
    channels: usize,
}

impl CallbackState {
    fn new(consumer: ringbuf::HeapCons<SfxHandle>, atlas: SoundAtlas, channels: usize) -> Self {
        Self {
            consumer,
            mixer: Mixer::new(),
            atlas,
            channels,
        }
    }

    #[inline(always)]
    fn mix_into(&mut self, data: &mut [f32]) {
        data.fill(0.0);

        while let Some(handle) = self.consumer.try_pop() {
            let clip = unsafe { self.atlas.clip_unchecked(handle) };
            let _ = self.mixer.add_sound(clip);
        }

        self.mixer.mix(self.channels, data);
    }
}

pub struct Player {
    producer: ringbuf::HeapProd<SfxHandle>,
    consumer: Option<ringbuf::HeapCons<SfxHandle>>,
    stream: Option<cpal::Stream>,
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
        self.stream = None;
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

    fn build_stream(&mut self) -> anyhow::Result<()> {
        if self.cached_sources.is_none() {
            return Ok(());
        }

        self.drop_stream();
        self.reset_queue();

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device"))?;
        let supported_config = device.default_output_config()?;
        self.device_sample_rate = supported_config.sample_rate();
        let config: cpal::StreamConfig = supported_config.into();

        let channels = config.channels as usize;

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
        let mut callback_state = CallbackState::new(consumer, atlas, channels);

        let device_lost_trigger = Arc::clone(&self.device_lost);
        device_lost_trigger.store(false, Ordering::Release);

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                callback_state.mix_into(data);
            },
            move |_| {
                device_lost_trigger.store(true, Ordering::Release);
            },
            None,
        )?;

        stream.play()?;
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
