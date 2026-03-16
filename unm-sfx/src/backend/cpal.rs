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
    player::{PlayCommand, COMMAND_QUEUE_CAPACITY},
};

struct CallbackState {
    consumer: ringbuf::HeapCons<PlayCommand>,
    mixer: Mixer,
    atlas: SoundAtlas,
    pending_commands: [PlayCommand; COMMAND_QUEUE_CAPACITY],
    channels: usize,
    loaded_sound_count: usize,
}

impl CallbackState {
    fn new(
        consumer: ringbuf::HeapCons<PlayCommand>,
        atlas: SoundAtlas,
        channels: usize,
        loaded_sound_count: usize,
    ) -> Self {
        Self {
            consumer,
            mixer: Mixer::new(),
            atlas,
            pending_commands: [PlayCommand::default(); COMMAND_QUEUE_CAPACITY],
            channels,
            loaded_sound_count,
        }
    }

    #[inline(always)]
    fn enqueue_single(&mut self, handle: SfxHandle) {
        if !handle.is_valid_for_len(self.loaded_sound_count) {
            return;
        }

        let clip = unsafe { self.atlas.clip_unchecked(handle) };
        let _ = self.mixer.add_sound(clip);
    }

    #[inline(always)]
    fn enqueue_repeat(&mut self, handle: SfxHandle, count: u16) {
        if count == 0 || !handle.is_valid_for_len(self.loaded_sound_count) {
            return;
        }

        let clip = unsafe { self.atlas.clip_unchecked(handle) };
        for _ in 0..count {
            if !self.mixer.add_sound(clip) {
                break;
            }
        }
    }

    #[inline(always)]
    fn mix_into(&mut self, data: &mut [f32]) {
        loop {
            let queued = self.consumer.pop_slice(&mut self.pending_commands);
            if queued == 0 {
                break;
            }

            for idx in 0..queued {
                let command = self.pending_commands[idx];
                match command {
                    PlayCommand::None => {}
                    PlayCommand::Single(handle) => self.enqueue_single(handle),
                    PlayCommand::Repeat { handle, count } => self.enqueue_repeat(handle, count),
                }
            }
        }

        self.mixer.mix(self.channels, data);
    }
}

pub struct Player {
    producer: ringbuf::HeapProd<PlayCommand>,
    consumer: Option<ringbuf::HeapCons<PlayCommand>>,
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

    fn new_queue() -> (ringbuf::HeapProd<PlayCommand>, ringbuf::HeapCons<PlayCommand>) {
        HeapRb::<PlayCommand>::new(COMMAND_QUEUE_CAPACITY).split()
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

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device"))?;
        let supported_config = device.default_output_config()?;
        self.device_sample_rate = supported_config.sample_rate();
        let config = supported_config.config();

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
        let loaded_sound_count = self.loaded_sound_count();
        let mut callback_state = CallbackState::new(consumer, atlas, channels, loaded_sound_count);

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

        let _ = self.producer.try_push(PlayCommand::Single(handle));
    }

    fn submit_frame_play_count(&mut self, handle: SfxHandle, count: u16) {
        if count == 0 || !handle.is_valid_for_len(self.loaded_sound_count()) {
            return;
        }

        let _ = self
            .producer
            .try_push(PlayCommand::Repeat { handle, count });
    }
}
