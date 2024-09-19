use crate::{Reader, Writer};

use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

const POOL_SIZE: usize = 3;

struct Inner<T> {
    pool: [UnsafeCell<T>; POOL_SIZE],
    free: [AtomicBool; POOL_SIZE],
    // either -1 or in [0, POOL_SIZE)
    buffer: AtomicIsize,
}

/// This is a Single-Producer/Single-Consumer data structure so we must follow these laws:
/// 1. both `ReadHandle` and `WriteHandle` must point to the same `Inner` struct
/// 2. only one thread can **own** or **reference** a `ReadHandle`
/// 3. only one thread can **own** or **reference** a `WriteHandle`

/// In order to comply, `Sync` and `Send` traits must be carefully handled.
///
/// 1. `Inner` should implemented `Sync` (but only if T is `Send`)
/// 2. `ReadHandle` should **not** implement `Sync`, but allow `Send`
/// 3. `WriteHandle` should **not** implement `Sync`, but allow `Send`
///
/// These requires negative trait bounds which are not yet implemented.
/// For now add `_unimpl_sync` as `PhantomData<Cell>` to both `ReadHandle` and `WriteHandle` in order to
/// avoid auto implementation of Sync trait for them.

unsafe impl<T> Sync for Inner<T> where T: Send {}

pub struct ReadHandle<T> {
    inner: Arc<Inner<T>>,
    _unimpl_sync: PhantomData<Cell<()>>,
}

pub struct WriteHandle<T> {
    inner: Arc<Inner<T>>,
    _unimpl_sync: PhantomData<Cell<()>>,
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
    /// This method is wait-free since there is always a spot in the pool where we can write to.
    fn write(&self, value: T) {
        let idx = self.acquire();
        self.write_to(idx, value);
        // Safety: this is fine, idx can only be in [0, POOL_SIZE)
        let buffer = self.buffer.swap(idx as isize, Ordering::AcqRel);
        if buffer != -1 {
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
        unsafe {
            let pool = self.pool.get_unchecked(idx).get();
            (*pool).clone()
        }
    }

    /// Returns the index of the first available object in the pool, while marking it as in use.
    /// It is assumed that at least one object is always free.
    fn acquire(&self) -> usize {
        for idx in 0..POOL_SIZE {
            let free = self.free[idx].swap(false, Ordering::AcqRel);
            if free {
                return idx;
            }
        }
        unreachable!()
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
        _unimpl_sync: std::marker::PhantomData,
    };
    let w = WriteHandle {
        inner: Arc::clone(&inner),
        _unimpl_sync: std::marker::PhantomData,
    };
    (r, w)
}
