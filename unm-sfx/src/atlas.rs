use crate::clip::{ClipMap, SfxHandle, MAX_SOUND_COUNT};

pub struct RawSource {
    pub data: Box<[f32]>,
    pub sample_rate: u32,
    pub frames_count: usize,
}

pub struct SoundAtlas {
    _buffer: Box<[f32]>,
    clips: Box<[ClipMap]>,
}

impl SoundAtlas {
    pub fn build_from_sources(sources: &[RawSource], device_sample_rate: u32) -> Self {
        debug_assert!(sources.len() <= MAX_SOUND_COUNT);

        let mut central_data = Vec::new();
        let mut clip_offsets = Vec::with_capacity(sources.len());

        for source in sources {
            let processed_samples = if source.sample_rate != device_sample_rate {
                Self::perform_resample(source, device_sample_rate)
            } else {
                source.data.to_vec()
            };

            while central_data.len() % 16 != 0 {
                central_data.push(0.0);
            }

            let offset = central_data.len();
            let frames = processed_samples.len();
            central_data.extend(processed_samples);
            clip_offsets.push((offset, frames));
        }

        let final_buffer = central_data.into_boxed_slice();
        let base_ptr = final_buffer.as_ptr();
        let clips = clip_offsets
            .into_iter()
            .map(|(offset, frames)| ClipMap {
                data_ptr: unsafe { base_ptr.add(offset) },
                frames_count: frames,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            _buffer: final_buffer,
            clips,
        }
    }

    #[inline(always)]
    pub unsafe fn clip_unchecked(&self, handle: SfxHandle) -> ClipMap {
        *self.clips.get_unchecked(handle.index_unchecked())
    }

    fn perform_resample(source: &RawSource, target_rate: u32) -> Vec<f32> {
        let duration = source.frames_count as f32 / source.sample_rate as f32;
        let target_frames_count = (duration * target_rate as f32).ceil() as usize;
        let mut new_data = Vec::with_capacity(target_frames_count);

        for i in 0..target_frames_count {
            let time = i as f32 / target_rate as f32;
            let sample = Self::lerp_sample_from_raw(source, time);
            new_data.push(sample);
        }

        new_data
    }

    fn lerp_sample_from_raw(source: &RawSource, time: f32) -> f32 {
        let idxf32 = time * source.sample_rate as f32;
        let idx = idxf32 as usize;
        let fract = idxf32 - idx as f32;

        let curr = Self::get_raw_frame(source, idx);
        let next = Self::get_raw_frame(source, idx + 1);

        curr + fract * (next - curr)
    }

    #[inline(always)]
    fn get_raw_frame(source: &RawSource, frame_idx: usize) -> f32 {
        if frame_idx < source.frames_count {
            source.data[frame_idx]
        } else {
            0.0
        }
    }
}
