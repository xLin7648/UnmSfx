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

    #[inline(always)]
    unsafe fn mix_frame(src_ptr: *const f32, src_channels: usize, out_ptr: *mut f32, out_channels: usize) {
        match (src_channels, out_channels) {
            (1, 1) => {
                *out_ptr += *src_ptr;
            }
            (1, _) => {
                let v = *src_ptr;
                for out_channel in 0..out_channels {
                    *out_ptr.add(out_channel) += v;
                }
            }
            (_, 1) => {
                let mut mono = 0.0;
                for src_channel in 0..src_channels {
                    mono += *src_ptr.add(src_channel);
                }
                *out_ptr += mono / src_channels as f32;
            }
            (2, 2) => {
                *out_ptr += *src_ptr;
                *out_ptr.add(1) += *src_ptr.add(1);
            }
            (2, _) => {
                let left = *src_ptr;
                let right = *src_ptr.add(1);
                *out_ptr += left;
                *out_ptr.add(1) += right;

                let mid = (left + right) * 0.5;
                for out_channel in 2..out_channels {
                    *out_ptr.add(out_channel) += mid;
                }
            }
            (_, 2) => {
                let left = *src_ptr;
                let right = *src_ptr.add(1);

                if src_channels == 2 {
                    *out_ptr += left;
                    *out_ptr.add(1) += right;
                    return;
                }

                let mut residual = 0.0;
                for src_channel in 2..src_channels {
                    residual += *src_ptr.add(src_channel);
                }
                residual /= (src_channels - 2) as f32;

                *out_ptr += left + residual * 0.5;
                *out_ptr.add(1) += right + residual * 0.5;
            }
            _ => {
                let common = src_channels.min(out_channels);
                for channel in 0..common {
                    *out_ptr.add(channel) += *src_ptr.add(channel);
                }

                if out_channels > src_channels {
                    let mut mono = 0.0;
                    for src_channel in 0..src_channels {
                        mono += *src_ptr.add(src_channel);
                    }
                    mono /= src_channels as f32;

                    for out_channel in src_channels..out_channels {
                        *out_ptr.add(out_channel) += mono;
                    }
                } else if src_channels > out_channels {
                    let mut residual = 0.0;
                    for src_channel in out_channels..src_channels {
                        residual += *src_ptr.add(src_channel);
                    }
                    let spread = (residual / (src_channels - out_channels) as f32) / out_channels as f32;

                    for out_channel in 0..out_channels {
                        *out_ptr.add(out_channel) += spread;
                    }
                }
            }
        }
    }

    pub(crate) fn mix(&mut self, channels: usize, out_data: &mut [f32]) {
        if channels == 0 || out_data.is_empty() {
            return;
        }

        out_data.fill(0.0);

        let sounds = &mut self.0;
        if sounds.is_empty() {
            return;
        }

        let out_frames = out_data.len() / channels;
        if out_frames == 0 {
            return;
        }

        let out_ptr = out_data.as_mut_ptr();

        let mut i = 0;
        while i < sounds.len() {
            let sound = unsafe { sounds.get_unchecked_mut(i) };
            let mix_frames = out_frames.min(sound.clip.frames_count - sound.cursor);

            if mix_frames == 0 {
                sounds.swap_remove(i);
                continue;
            }

            let src_channels = sound.clip.channel_count;
            let src_base = unsafe { sound.clip.data_ptr.add(sound.cursor * src_channels) };

            for frame in 0..mix_frames {
                unsafe {
                    let src_ptr = src_base.add(frame * src_channels);
                    let dst_ptr = out_ptr.add(frame * channels);
                    Self::mix_frame(src_ptr, src_channels, dst_ptr, channels);
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
