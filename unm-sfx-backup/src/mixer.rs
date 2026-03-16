use crate::clip::{ClipMap, MAX_ACTIVE_SOUNDS};

struct SoundState {
    clip: ClipMap,
    cursor: usize,
}

pub(crate) struct Mixer(Vec<SoundState>);

impl Mixer {
    pub(crate) fn new() -> Self {
        Self(Vec::with_capacity(MAX_ACTIVE_SOUNDS))
    }

    #[inline(always)]
    pub(crate) fn add_sound(&mut self, clip: ClipMap) -> bool {
        if self.0.len() >= MAX_ACTIVE_SOUNDS {
            return false;
        }

        self.0.push(SoundState { clip, cursor: 0 });
        true
    }

    pub(crate) fn mix(&mut self, channels: usize, out_data: &mut [f32]) {
        let sounds = &mut self.0;
        if sounds.is_empty() {
            return;
        }

        let out_frames = out_data.len() / channels;
        let out_ptr = out_data.as_mut_ptr();
        let mut i = 0;

        while i < sounds.len() {
            let sound = unsafe { sounds.get_unchecked_mut(i) };
            let mix_frames = out_frames.min(sound.clip.frames_count - sound.cursor);

            if mix_frames == 0 {
                sounds.swap_remove(i);
                continue;
            }

            unsafe {
                let src_ptr = sound.clip.data_ptr.add(sound.cursor);

                match channels {
                    1 => {
                        for j in 0..mix_frames {
                            *out_ptr.add(j) += *src_ptr.add(j);
                        }
                    }
                    2 => {
                        for j in 0..mix_frames {
                            let mono_sample = *src_ptr.add(j);
                            let out_base_idx = j * 2;
                            *out_ptr.add(out_base_idx) += mono_sample;
                            *out_ptr.add(out_base_idx + 1) += mono_sample;
                        }
                    }
                    _ => {
                        for j in 0..mix_frames {
                            let mono_sample = *src_ptr.add(j);
                            let out_frame_base_idx = j * channels;
                            for c in 0..channels {
                                *out_ptr.add(out_frame_base_idx + c) += mono_sample;
                            }
                        }
                    }
                }
            }

            sound.cursor += mix_frames;

            if sound.cursor >= sound.clip.frames_count {
                sounds.swap_remove(i);
            } else {
                i += 1;
            }
        }

        for sample in out_data.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}
