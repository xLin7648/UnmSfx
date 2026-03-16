pub const MAX_SOUND_COUNT: usize = 14;
pub const MAX_ACTIVE_SOUNDS: usize = 256;

#[derive(Default, Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub struct SfxHandle(pub u8);

impl SfxHandle {
    #[inline(always)]
    pub const fn from_index(index: usize) -> Self {
        Self(index as u8 + 1)
    }

    #[inline(always)]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    #[inline(always)]
    pub const fn index_unchecked(self) -> usize {
        self.0 as usize - 1
    }

    #[inline(always)]
    pub fn is_valid_for_len(self, len: usize) -> bool {
        self.is_valid() && self.index_unchecked() < len
    }
}

unsafe impl Send for SfxHandle {}
unsafe impl Sync for SfxHandle {}

#[derive(Clone, Copy)]
pub(crate) struct ClipMap {
    pub data_ptr: *const f32,
    pub frames_count: usize,
}

unsafe impl Send for ClipMap {}
unsafe impl Sync for ClipMap {}
