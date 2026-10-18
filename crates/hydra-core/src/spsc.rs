use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Cache-line padded wrapper (64 bytes) to eliminate false sharing between Producer and Consumer cores.
#[repr(align(64))]
#[derive(Debug)]
struct CachePaddedAtomicUsize(AtomicUsize);

impl CachePaddedAtomicUsize {
    #[inline(always)]
    const fn new(v: usize) -> Self {
        Self(AtomicUsize::new(v))
    }
}

/// Lock-free Single-Producer Single-Consumer (SPSC) bounded ring buffer.
///
/// Designed for sub-50ns inter-thread message passing between the WebSocket / UDP reader thread
/// and the pinned Graph Solver / Execution thread with zero heap allocations after initialization.
///
/// `N` MUST be a power of two (e.g., 256, 1024, 4096).
pub struct SpscQueue<T, const N: usize> {
    head: CachePaddedAtomicUsize,
    tail: CachePaddedAtomicUsize,
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
}

unsafe impl<T: Send, const N: usize> Send for SpscQueue<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for SpscQueue<T, N> {}

impl<T, const N: usize> SpscQueue<T, N> {
    const MASK: usize = N - 1;

    /// Creates a new pre-allocated lock-free SPSC ring buffer.
    /// Panics at construction time if `N` is not a power of 2 or is `< 2`.
    pub fn new() -> Self {
        assert!(N >= 2 && N.is_power_of_two(), "SpscQueue capacity N must be a power of 2");
        let buffer = std::array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit()));
        Self {
            head: CachePaddedAtomicUsize::new(0),
            tail: CachePaddedAtomicUsize::new(0),
            buffer,
        }
    }

    /// Pushes an item onto the ring buffer without blocking or allocating.
    /// Returns `Err(item)` if the ring buffer is full.
    #[inline(always)]
    pub fn try_push(&self, item: T) -> Result<(), T> {
        let head = self.head.0.load(Ordering::Relaxed);
        let tail = self.tail.0.load(Ordering::Acquire);

        if head.wrapping_sub(tail) >= N {
            return Err(item);
        }

        let slot = head & Self::MASK;
        unsafe {
            (*self.buffer[slot].get()).write(item);
        }

        self.head.0.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Pops an item from the ring buffer without blocking or allocating.
    /// Returns `None` if the ring buffer is empty.
    #[inline(always)]
    pub fn try_pop(&self) -> Option<T> {
        let tail = self.tail.0.load(Ordering::Relaxed);
        let head = self.head.0.load(Ordering::Acquire);

        if tail == head {
            return None;
        }

        let slot = tail & Self::MASK;
        let item = unsafe { (*self.buffer[slot].get()).assume_init_read() };

        self.tail.0.store(tail.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    /// Returns the approximate number of queued items.
    #[inline(always)]
    pub fn len(&self) -> usize {
        let head = self.head.0.load(Ordering::Acquire);
        let tail = self.tail.0.load(Ordering::Acquire);
        head.wrapping_sub(tail)
    }

    /// Returns `true` if the queue is currently empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the usable capacity of the ring buffer.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }
}

impl<T, const N: usize> Default for SpscQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for SpscQueue<T, N> {
    fn drop(&mut self) {
        while self.try_pop().is_some() {}
    }
}
