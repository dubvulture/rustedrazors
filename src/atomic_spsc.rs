use crate::{ReadGuard, ReadState, Reader, Writer};

use std::cell::{Cell, UnsafeCell};
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

const POOL_SIZE: usize = 3;

struct Inner<T> {
    pool: [UnsafeCell<T>; POOL_SIZE],
    free: [AtomicBool; POOL_SIZE],
    // either -1 or in [0, POOL_SIZE)
    buffer: AtomicIsize,
    old_read_idx: UnsafeCell<usize>,
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
            old_read_idx: UnsafeCell::new(0),
        }
    }
}

impl<T> Inner<T> {
    /// Writes the provided value.
    ///
    /// This method is wait-free since there is always a spot in the pool where we can write to.
    fn write(&self, value: T) {
        let idx = self.acquire();
        self.write_to(idx, value);
        // Safety: this is fine, idx can only be in [0, POOL_SIZE)
        let buffer = self.buffer.swap(idx as isize, Ordering::AcqRel);
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
    /// The operation may return the previously read value if no new value was written since the
    /// last read.
    ///
    /// This method is wait-free.
    fn read(&self) -> Guard<'_, T> {
        // SAFETY: Single Consumer, this is only used by a single reader
        let old_read_idx = unsafe { *self.old_read_idx.get() };
        let buffer = self.buffer.load(Ordering::Relaxed);
        if buffer < 0 {
            // Nothing to read, reuse old read index, without releasing it yet
            let data = self.read_from(old_read_idx);
            Guard {
                data,
                state: ReadState::Stale,
            }
        } else {
            // Release before performing the swap, otherwise we might starve the Writer.
            self.release(old_read_idx);
            // Swap just in case the Writer has updated the value since the last load
            let buffer = self.buffer.swap(-1, Ordering::AcqRel);
            // SAFETY: Here, buffer can only be in [0, POOL_SIZE)
            let new_read_idx = buffer as usize;
            // SAFETY: Single Consumer, this is only used by a single reader
            unsafe {
                *self.old_read_idx.get() = new_read_idx;
            };
            let data = self.read_from(new_read_idx);
            Guard {
                data,
                state: ReadState::Fresh,
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

pub struct Guard<'a, T> {
    data: &'a T,
    state: ReadState,
}

impl<'a, T> ReadGuard<'a, T> for Guard<'a, T> {
    fn state(&self) -> ReadState {
        self.state
    }
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

impl<T> fmt::Debug for Guard<'_, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T> Reader for ReadHandle<T> {
    type Item = T;
    type Guard<'a>
        = Guard<'a, T>
    where
        T: 'a;

    fn read(&self) -> Self::Guard<'_> {
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
        _unimpl_sync: std::marker::PhantomData,
    };
    let w = WriteHandle {
        inner: Arc::clone(&inner),
        _unimpl_sync: std::marker::PhantomData,
    };
    (r, w)
}
