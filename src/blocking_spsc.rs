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

    /// Writes the provided value.
    ///
    /// This method is not wait-free since there is not always a spot in the pool where we can write to.
    fn write(&self, value: T) {
        let mut idx = -1;
        for i in 0.. {
            idx = self.acquire();
            if idx != -1 {
                break;
            }
            if i >= 20 {
                std::thread::yield_now();
            }
        }
        self.write_to(idx as usize, value);
        // Safety: this is fine, idx can only be in [0, POOL_SIZE)
        let buffer = self.buffer.swap(idx, Ordering::AcqRel);
        if buffer != -1 {
            self.release(buffer as usize);
        }
    }

    fn write_to(&self, idx: usize, value: T) {
        let pool = self.pool[idx].get();
        unsafe { *pool = value }
    }

    /// Try reading the last written value.
    /// The operation may fail if no new value was written since the last read.
    ///
    /// This method is wait-free.
    fn read(&self) -> Option<T> {
        let buffer = self.buffer.swap(-1, Ordering::AcqRel);
        match buffer {
            -1 => None,
            buffer => {
                // Safety: this is fine, idx can only be in [0, POOL_SIZE)
                let buffer = buffer as usize;
                let value = self.read_from(buffer);
                self.release(buffer);
                Some(value)
            }
        }
    }

    fn read_from(&self, idx: usize) -> T {
        let pool = self.pool[idx].get();
        unsafe { (*pool).clone() }
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

impl<T> Reader<T> for ReadHandle<T>
where
    T: Clone,
{
    fn read(&self) -> Option<T> {
        self.inner.read()
    }
}

impl<T> Writer<T> for WriteHandle<T>
where
    T: Clone,
{
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
