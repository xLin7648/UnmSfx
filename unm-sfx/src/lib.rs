use std::{cell::UnsafeCell, sync::OnceLock};

use crate::{
    clip::{MAX_SOUND_COUNT, SfxHandle},
    player::SfxManager,
};

pub mod clip;
pub mod player;

mod atlas;
mod backend;
mod decoder;
mod mixer;

const ERR_MANAGER_NOT_INIT: i32 = -1;
const ERR_LOAD_FAILED: i32 = -2;
const ERR_INVALID_INPUT: i32 = -3;
const ERR_TOO_MANY_SOUNDS: i32 = -4;

struct ManagerSlot(UnsafeCell<Option<SfxManager>>);

impl ManagerSlot {
    fn global() -> &'static Self {
        static SLOT: OnceLock<ManagerSlot> = OnceLock::new();
        SLOT.get_or_init(|| ManagerSlot(UnsafeCell::new(None)))
    }

    #[inline(always)]
    fn get_mut(&self) -> *mut Option<SfxManager> {
        self.0.get()
    }
}

unsafe impl Sync for ManagerSlot {}

#[inline(always)]
unsafe fn manager_mut() -> &'static mut Option<SfxManager> {
    &mut *ManagerSlot::global().get_mut()
}

// No Mutex / RwLock. Unity should call the control-path entrypoints on one thread.
#[no_mangle]
pub extern "C" fn unm_sfx_init() {
    unsafe {
        let manager = manager_mut();
        if manager.is_none() {
            *manager = Some(SfxManager::new());
        }
    }
}

#[no_mangle]
pub extern "C" fn unm_sfx_tick() {
    unsafe {
        if let Some(manager) = manager_mut().as_mut() {
            manager.maintain_stream();
        }
    }
}

#[no_mangle]
pub extern "C" fn unm_sfx_load_sound(
    data_ptrs: *const *const u8,
    data_lens: *const usize,
    count: usize,
    out_handles: *mut u8,
) -> i32 {
    if count == 0 || count > MAX_SOUND_COUNT {
        return if count > MAX_SOUND_COUNT {
            ERR_TOO_MANY_SOUNDS
        } else {
            ERR_INVALID_INPUT
        };
    }

    if data_ptrs.is_null() || data_lens.is_null() || out_handles.is_null() {
        return ERR_INVALID_INPUT;
    }

    unsafe {
        let manager = match manager_mut().as_mut() {
            Some(manager) => manager,
            None => return ERR_MANAGER_NOT_INIT,
        };

        let mut all_data = Vec::with_capacity(count);
        for index in 0..count {
            let ptr = *data_ptrs.add(index);
            let len = *data_lens.add(index);
            if ptr.is_null() || len == 0 {
                return ERR_INVALID_INPUT;
            }

            let slice = std::slice::from_raw_parts(ptr, len);
            all_data.push(slice.to_vec());
        }

        let handles = match manager.init_load_sound(all_data) {
            Some(handles) => handles,
            None => return ERR_LOAD_FAILED,
        };

        for (index, handle) in handles.iter().enumerate() {
            *out_handles.add(index) = handle.0;
        }
    }

    0
}

#[no_mangle]
pub extern "C" fn unm_sfx_load_pcm_f32(
    data_ptrs: *const *const f32,
    frame_counts: *const u32,
    channel_counts: *const u32,
    sample_rates: *const u32,
    count: usize,
    out_handles: *mut u8,
) -> i32 {
    if count == 0 || count > MAX_SOUND_COUNT {
        return if count > MAX_SOUND_COUNT {
            ERR_TOO_MANY_SOUNDS
        } else {
            ERR_INVALID_INPUT
        };
    }

    if data_ptrs.is_null()
        || frame_counts.is_null()
        || channel_counts.is_null()
        || sample_rates.is_null()
        || out_handles.is_null()
    {
        return ERR_INVALID_INPUT;
    }

    unsafe {
        let manager = match manager_mut().as_mut() {
            Some(manager) => manager,
            None => return ERR_MANAGER_NOT_INIT,
        };

        let mut sources = Vec::with_capacity(count);
        for index in 0..count {
            let data_ptr = *data_ptrs.add(index);
            let frames_count = *frame_counts.add(index) as usize;
            let channel_count = *channel_counts.add(index) as usize;
            let sample_rate = *sample_rates.add(index);

            if data_ptr.is_null() || frames_count == 0 || channel_count == 0 || sample_rate == 0 {
                return ERR_INVALID_INPUT;
            }

            let sample_count = match frames_count.checked_mul(channel_count) {
                Some(sample_count) => sample_count,
                None => return ERR_INVALID_INPUT,
            };
            let data = std::slice::from_raw_parts(data_ptr, sample_count);

            let source = match decoder::from_interleaved_f32(
                data,
                frames_count,
                channel_count,
                sample_rate,
            ) {
                Ok(source) => source,
                Err(_) => return ERR_LOAD_FAILED,
            };
            sources.push(source);
        }

        let handles = match manager.init_load_sound_from_sources(sources) {
            Some(handles) => handles,
            None => return ERR_LOAD_FAILED,
        };

        for (index, handle) in handles.iter().enumerate() {
            *out_handles.add(index) = handle.0;
        }
    }

    0
}

#[no_mangle]
pub extern "C" fn unm_sfx_play(handle: u8) {
    unsafe {
        if let Some(manager) = manager_mut().as_mut() {
            manager.play(SfxHandle(handle));
        }
    }
}

#[no_mangle]
pub extern "C" fn unm_sfx_submit_frame_play_count(handle: u8, count: u16) -> i32 {
    if count == 0 {
        return 0;
    }

    unsafe {
        let manager = match manager_mut().as_mut() {
            Some(manager) => manager,
            None => return ERR_MANAGER_NOT_INIT,
        };
        manager.submit_frame_play_count(SfxHandle(handle), count);
    }

    0
}

#[no_mangle]
pub extern "C" fn unm_sfx_shutdown() {
    unsafe {
        let manager = manager_mut();
        if let Some(mut manager) = manager.take() {
            manager.shutdown();
        }
    }
}

#[cfg(target_os = "android")]
#[link(name = "c++_shared")]
extern "C" {}
