use std::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use std::ptr::NonNull;
use std::sync::{LazyLock, Mutex};

const CHUNK_SIZE: usize = 64 * 1024;

pub struct Chunk {
    ptr: NonNull<u8>,
    capacity: usize,
    offset: usize,
}

impl Chunk {
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 64).unwrap();
        let ptr = unsafe { alloc(layout) };

        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            capacity,
            offset: 0,
        }
    }

    #[inline]
    pub fn alloc_raw(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let base = self.ptr.as_ptr() as usize;
        let current = base + self.offset;
        let aligned = (current + align - 1) & !(align - 1);
        let next = aligned + size;

        if next > base + self.capacity {
            return None;
        }

        self.offset = next - base;

        Some(aligned as *mut u8)
    }

    #[inline]
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// The chunk exclusively owns its allocation, moving it across threads is fine.
unsafe impl Send for Chunk {}

impl Drop for Chunk {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.ptr.as_ptr(),
                Layout::from_size_align(self.capacity, 64).unwrap(),
            );
        }
    }
}

pub struct Arena {
    chunks: Vec<Chunk>,
    chunk_size: usize,
}

impl Arena {
    /// Chunks are allocated on first use, an unused arena costs nothing.
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunks: Vec::new(),
            chunk_size,
        }
    }

    pub fn alloc_raw(&mut self, size: usize, align: usize) -> *mut u8 {
        // Aligned but dangling, never read: the caller has a zero length.
        if size == 0 {
            return align as *mut u8;
        }

        loop {
            if let Some(chunk) = self.chunks.last_mut()
                && let Some(ptr) = chunk.alloc_raw(size, align)
            {
                return ptr;
            }

            let capacity = self.chunk_size.max(size);
            self.chunks.push(Chunk::new(capacity));
        }
    }

    #[inline]
    pub fn alloc<T>(&mut self, len: usize) -> *mut T {
        self.alloc_raw(std::mem::size_of::<T>() * len, std::mem::align_of::<T>()) as *mut T
    }

    pub fn reset(&mut self) {
        for chunk in &mut self.chunks {
            chunk.reset();
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

pub struct ArenaPool {
    free: Vec<Arena>,
    chunk_size: usize,
}

impl ArenaPool {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            free: Vec::new(),
            chunk_size,
        }
    }

    pub fn acquire(&mut self) -> Arena {
        self.free
            .pop()
            .unwrap_or_else(|| Arena::new(self.chunk_size))
    }

    pub fn release(&mut self, mut arena: Arena) {
        arena.reset();
        self.free.push(arena);
    }

    pub fn cached(&self) -> usize {
        self.free.len()
    }
}

static POOL: LazyLock<Mutex<ArenaPool>> = LazyLock::new(|| Mutex::new(ArenaPool::new(CHUNK_SIZE)));

/// Takes an arena from the process wide pool.
pub fn acquire() -> Arena {
    POOL.lock().unwrap().acquire()
}

/// Returns an arena to the pool, keeping its chunks for the next user.
pub fn release(arena: Arena) {
    POOL.lock().unwrap().release(arena);
}

pub fn cached() -> usize {
    POOL.lock().unwrap().cached()
}
