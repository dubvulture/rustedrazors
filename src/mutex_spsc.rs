use crate::{Reader, Writer};

/// Implement a trivial atomic_spsc-like data structures using a Mutex
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

struct Inner<T> {
    data: Mutex<T>,
    to_read: AtomicBool,
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
            to_read: AtomicBool::new(false),
        }
    }

    fn write(&self, value: T) {
        let mut data = self.data.lock().unwrap();
        *data = value;
        self.to_read.store(true, Ordering::Release);
    }

    fn read(&self) -> Option<MutexGuard<'_, T>> {
        if self.to_read.load(Ordering::Acquire) {
            let guard = self.data.lock().ok()?;
            self.to_read.store(false, Ordering::Release);
            Some(guard)
        } else {
            None
        }
    }
}

impl<T> Reader for ReadHandle<T> {
    type Item = T;
    type Guard<'a>
        = MutexGuard<'a, T>
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
