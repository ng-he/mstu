use std::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread_local;

/// Chunk a message's fields and small values are cut from.
const CHUNK_SIZE: usize = 64 * 1024;

/// Chunks at least this big are worth a huge page: a video frame spans dozens
/// of normal pages, and every one of them is a TLB entry.
const HUGE_PAGE_THRESHOLD: usize = 512 * 1024;

/// Arenas kept per thread. Enough for the messages one node has in flight,
/// not so many that a burst parks memory for good.
const POOL_LIMIT: usize = 32;

/// Chunks above this are freed instead of being kept: one oversized frame
/// must not hold its memory for the rest of the run.
const KEEP_CHUNK_LIMIT: usize = 4 * CHUNK_SIZE;

static LIVE_CHUNKS: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

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

        LIVE_CHUNKS.fetch_add(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_add(capacity, Ordering::Relaxed);

        if capacity >= HUGE_PAGE_THRESHOLD {
            advise_huge(ptr, capacity);
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

/// Asks the kernel to back a big allocation with huge pages, and shrugs if it
/// will not: it is a hint, and the memory is just as usable without it.
#[cfg(target_os = "linux")]
fn advise_huge(ptr: *mut u8, len: usize) {
    const MADV_HUGEPAGE: i32 = 14;

    unsafe extern "C" {
        fn madvise(addr: *mut std::ffi::c_void, len: usize, advice: i32) -> i32;
    }

    unsafe { madvise(ptr.cast(), len, MADV_HUGEPAGE) };
}

#[cfg(not(target_os = "linux"))]
fn advise_huge(_ptr: *mut u8, _len: usize) {}

/// The chunk exclusively owns its allocation, moving it across threads is fine.
unsafe impl Send for Chunk {}

impl Drop for Chunk {
    fn drop(&mut self) {
        LIVE_CHUNKS.fetch_sub(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_sub(self.capacity, Ordering::Relaxed);

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

    /// Frees nothing, only winds the chunks back to the start.
    pub fn reset(&mut self) {
        for chunk in &mut self.chunks {
            chunk.reset();
        }
    }

    /// Winds back and gives up whatever was oversized, so one big message does
    /// not park its memory in the pool for the rest of the run.
    pub fn trim(&mut self) {
        self.chunks
            .retain(|chunk| chunk.capacity() <= KEEP_CHUNK_LIMIT);
        self.reset();
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Whether `ptr` points into this arena, so a value already lives here and
    /// needs no copy. Chunks are few, so this is a walk of one or two ranges.
    pub fn owns(&self, ptr: *const u8) -> bool {
        let address = ptr as usize;

        self.chunks.iter().any(|chunk| {
            let base = chunk.ptr.as_ptr() as usize;

            address >= base && address < base + chunk.capacity
        })
    }
}

/// Per thread, so the router and the nodes never wait on each other for memory.
///
/// An arena acquired on one thread may be released on another, which only
/// moves it between pools.
struct Pool {
    free: Vec<Arena>,
}

thread_local! {
    static POOL: RefCell<Pool> = const { RefCell::new(Pool { free: Vec::new() }) };
}

/// Takes an arena from this thread's pool.
pub fn acquire() -> Arena {
    POOL.with(|pool| pool.borrow_mut().free.pop())
        .unwrap_or_else(|| Arena::new(CHUNK_SIZE))
}

/// Returns an arena to this thread's pool, keeping its chunks for the next
/// message. A pool that is already full drops it instead.
pub fn release(mut arena: Arena) {
    arena.trim();

    POOL.with(|pool| {
        let mut pool = pool.borrow_mut();

        if pool.free.len() < POOL_LIMIT {
            pool.free.push(arena);
        }
    });
}

/// Arenas this thread is holding for reuse.
pub fn cached() -> usize {
    POOL.with(|pool| pool.borrow().free.len())
}

/// Chunks alive across the whole process, and what they add up to.
pub fn live() -> (usize, usize) {
    (
        LIVE_CHUNKS.load(Ordering::Relaxed),
        LIVE_BYTES.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pooled arena keeps its ordinary chunks and gives up the big ones.
    #[test]
    fn trims_oversized_chunks() {
        let mut arena = Arena::new(CHUNK_SIZE);

        arena.alloc::<u8>(1024);
        arena.alloc::<u8>(KEEP_CHUNK_LIMIT + 1);

        assert_eq!(arena.chunk_count(), 2);

        arena.trim();

        assert_eq!(arena.chunk_count(), 1);
    }

    /// Reuse is the point: the second message gets the first one's memory.
    #[test]
    fn hands_the_same_arena_back_out() {
        let mut arena = acquire();
        let first = arena.alloc::<u8>(64);

        release(arena);

        let mut again = acquire();
        assert_eq!(again.alloc::<u8>(64), first);

        release(again);
    }

    /// A burst must not park memory for the rest of the run.
    #[test]
    fn keeps_the_pool_bounded() {
        let held: Vec<Arena> = (0..POOL_LIMIT + 8).map(|_| acquire()).collect();

        for arena in held {
            release(arena);
        }

        assert_eq!(cached(), POOL_LIMIT);
    }
}
