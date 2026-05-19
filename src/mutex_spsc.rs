use crate::{Reader, Writer};

use std::fmt;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Implement a trivial atomic_spsc-like data structures using a Mutex

struct Inner<T> {
    data: Mutex<T>,
    dirty: AtomicBool,
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
            data: Mutex::new(init),
            dirty: AtomicBool::new(false),
        }
    }

    fn write(&self, value: T) {
        // acquire mutex, update value and set it as dirty
        let mut guard = self.data.lock().unwrap();
        *guard = value;
        self.dirty.store(true, Ordering::Release);
    }

    fn read(&self) -> Option<Guard<'_, T>> {
        // return guarded value only if dirty, while "cleaning" it if acquired
        self.dirty
            .swap(false, Ordering::AcqRel)
            .then_some(Guard(self.data.lock().unwrap()))
    }
}

/// This is only needed because we want to disallow DerefMut
pub struct Guard<'a, T>(MutexGuard<'a, T>);

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
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
