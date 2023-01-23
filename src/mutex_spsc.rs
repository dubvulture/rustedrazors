use crate::{Reader, Writer};

/// Implement a trivial atomic_spsc-like data structures using a Mutex
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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

impl<T> Inner<T>
where
    T: Clone,
{
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

    fn read(&self, value: &mut T) -> bool {
        if self.to_read.load(Ordering::Acquire) {
            let data = self.data.lock().unwrap();
            *value = T::clone(&data);
            self.to_read.store(false, Ordering::Release);
            true
        } else {
            false
        }
    }
}

impl<T> Reader<T> for ReadHandle<T>
where
    T: Clone,
{
    fn read(&self, value: &mut T) -> bool {
        self.inner.read(value)
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
