use crate::{Reader, Writer};

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

const POOL_SIZE: usize = 2;

struct Inner<T> {
    pool: [UnsafeCell<T>; POOL_SIZE],
    free: [AtomicBool; POOL_SIZE],
    // either -1 or in [0, POOL_SIZE)
    buffer: AtomicIsize,
}

/// Safety: enable SYnc when T is Send to allow sharing UnsafeCell.
/// UnsafeCell is accessed without data races by design.
unsafe impl<T> Sync for Inner<T> where T: Send {}

pub struct ReadHandle<T> {
    inner: Arc<Inner<T>>,
}

pub struct WriteHandle<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Inner<T>
where
    T: Clone,
{
    /// Constructs a new [`Inner`] initialized with the provided value.
    fn new(init: T) -> Self {
        Inner {
            pool: [(); POOL_SIZE].map(|_| UnsafeCell::new(init.clone())),
            free: [(); POOL_SIZE].map(|_| AtomicBool::new(true)),
            buffer: AtomicIsize::new(-1),
        }
    }
}

impl<T> Inner<T> {
    /// Writes the provided value.
    ///
    /// This method is not wait-free since there is not always a spot in the pool where we can write to.
    fn write(&self, value: T) {
        let mut idx = -1;
        for i in 0.. {
            idx = self.acquire();
            if idx >= 0 {
                break;
            }
            if i >= 20 {
                std::thread::yield_now();
            }
        }
        // Safety: this is fine, idx can only be in [0, POOL_SIZE)
        self.write_to(idx as usize, value);
        let buffer = self.buffer.swap(idx, Ordering::AcqRel);
        if buffer >= 0 {
            self.release(buffer as usize);
        }
    }

    fn write_to(&self, idx: usize, value: T) {
        unsafe {
            let pool = self.pool.get_unchecked(idx).get();
            *pool = value
        }
    }

    /// Try reading the last written value.
    /// The operation may fail if no new value was written since the last read.
    ///
    /// This method is wait-free.
    fn read(&self) -> Option<BlockingGuard<'_, T>> {
        let buffer = self.buffer.swap(-1, Ordering::AcqRel);
        match buffer {
            -1 => None,
            buffer => {
                // Safety: this is fine, idx can only be in [0, POOL_SIZE)
                let buffer = buffer as usize;
                let guard = BlockingGuard {
                    inner: self,
                    idx: buffer,
                };
                Some(guard)
            }
        }
    }

    fn read_from(&self, idx: usize) -> &T {
        unsafe {
            let pool = self.pool.get_unchecked(idx).get();
            &(*pool)
        }
    }

    /// Returns the index of the first available object in the pool, while marking it as in use.
    fn acquire(&self) -> isize {
        for idx in 0..POOL_SIZE {
            let free = self.free[idx].swap(false, Ordering::AcqRel);
            if free {
                return idx as isize;
            }
        }
        -1
    }

    /// Marks the object at the given index in the pool as free.
    fn release(&self, idx: usize) {
        self.free[idx].store(true, Ordering::Release);
    }
}

pub struct BlockingGuard<'a, T> {
    inner: &'a Inner<T>,
    idx: usize,
}

impl<T> std::ops::Deref for BlockingGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner.read_from(self.idx)
    }
}

impl<T> Drop for BlockingGuard<'_, T> {
    fn drop(&mut self) {
        self.inner.release(self.idx);
    }
}

impl<T> std::fmt::Debug for BlockingGuard<'_, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}

impl<T> Reader for ReadHandle<T> {
    type Item = T;
    type Guard<'a>
        = BlockingGuard<'a, T>
    where
        T: 'a;

    fn read(&self) -> Option<Self::Guard<'_>> {
        self.inner.read()
    }
}

impl<T> Writer for WriteHandle<T> {
    type Item = T;

    fn write(&self, value: T) {
        self.inner.write(value)
    }
}

/// Construct a new read and write handle pair from an data structure initialzied with `init`.
pub fn new<T>(init: T) -> (ReadHandle<T>, WriteHandle<T>)
where
    T: Clone,
{
    let inner = Arc::new(Inner::new(init));
    let r = ReadHandle {
        inner: Arc::clone(&inner),
    };
    let w = WriteHandle {
        inner: Arc::clone(&inner),
    };
    (r, w)
}
