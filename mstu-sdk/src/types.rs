use std::{
    ffi::{CStr, c_char},
    fmt,
};

/// A borrowed UTF-8 string.
///
/// `Str` does not own the underlying memory. The caller must ensure that
/// `ptr` points to a valid UTF-8 buffer of length `len`, and that the buffer
/// remains alive for the entire time `Str` is used.
///
/// This type is intended for FFI and is ABI-compatible with a pointer-length
/// string representation.
#[repr(C)]
#[derive(Copy, Clone)]

pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}

impl Str {
    pub fn new(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    pub const fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    /// An empty `Str` is a null pointer, which `from_raw_parts` rejects.
    pub unsafe fn as_str<'a>(&self) -> &'a str {
        if self.ptr.is_null() || self.len == 0 {
            return "";
        }

        unsafe {
            let bytes = std::slice::from_raw_parts(self.ptr, self.len);
            std::str::from_utf8_unchecked(bytes)
        }
    }

    pub const fn from_static(s: &'static str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { f.write_str(self.as_str()) }
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { fmt::Debug::fmt(self.as_str(), f) }
    }
}

unsafe impl Sync for Str {}
unsafe impl Send for Str {}

// ABI for Str
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_from_cstr(ptr: *const c_char) -> Str {
    if ptr.is_null() {
        return Str {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    let cstr = unsafe { CStr::from_ptr(ptr) };

    Str {
        ptr: cstr.as_ptr() as *const u8,
        len: cstr.to_bytes().len(),
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Slice<T> {
    pub ptr: *const T,
    pub len: usize,
}

impl<T> Slice<T> {
    pub const fn empty() -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
        }
    }

    pub const fn from_raw_parts(ptr: *const T, len: usize) -> Self {
        Self { ptr, len }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// An empty `Slice` is a null pointer, which `from_raw_parts` rejects.
    pub unsafe fn as_slice<'a>(&self) -> &'a [T] {
        if self.ptr.is_null() || self.len == 0 {
            return &[];
        }

        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        unsafe { Some(&*self.ptr.add(index)) }
    }

    #[inline]
    pub fn at(&self, index: usize) -> &T {
        assert!(index < self.len);
        unsafe { &*self.ptr.add(index) }
    }
}

#[macro_export]
macro_rules! slice {
    ($slice:expr) => {
        $crate::Slice::from_raw_parts(($slice).as_ptr(), ($slice).len())
    };
}

impl<T> From<&[T]> for Slice<T> {
    fn from(slice: &[T]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
        }
    }
}

impl<T> From<&mut [T]> for Slice<T> {
    fn from(slice: &mut [T]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
        }
    }
}
