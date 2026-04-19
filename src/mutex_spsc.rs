use crate::{ReadGuard, ReadState, Reader, Writer};

use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};

/// Implement a trivial atomic_spsc-like data structures using a Mutex

struct Inner<T> {
    data: Mutex<(T, ReadState)>,
}

pub struct ReadHandle<T> {
    inner: Arc<Inner<T>>,
}

pub struct WriteHandle<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Inner<T> {
    fn new(init: T) -> Self {
        Inner {
            data: Mutex::new((init, ReadState::Stale)),
        }
    }

    fn write(&self, value: T) {
        // acquire mutex, update value and set it as "Fresh"
        let mut guard = self.data.lock().unwrap();
        guard.0 = value;
        guard.1 = ReadState::Fresh;
    }

    fn read(&self) -> Guard<'_, T> {
        // acquire mutex and return it as custom guard that will set it as "Stale" after dropping
        let guard = self.data.lock().unwrap();
        Guard { guard }
    }
}

/// This is only needed because we want to disallow DerefMut and implement our custom Drop
pub struct Guard<'a, T> {
    guard: MutexGuard<'a, (T, ReadState)>,
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard.0
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

/// This is needed to set ReadState to Stale when dropping the Guard
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.guard.1 = ReadState::Stale;
    }
}

impl<'a, T> ReadGuard<'a, T> for Guard<'a, T> {
    fn state(&self) -> ReadState {
        self.guard.1
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

pub fn new<T>(init: T) -> (ReadHandle<T>, WriteHandle<T>) {
    let inner = Arc::new(Inner::new(init));
    let r = ReadHandle {
        inner: Arc::clone(&inner),
    };
    let w = WriteHandle {
        inner: Arc::clone(&inner),
    };
    (r, w)
}
