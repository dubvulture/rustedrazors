/// Implement a trivial atomic_spsc-like data structures using a Mutex
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct Inner<T> {
    data: T,
}

struct Shared<T> {
    inner: Mutex<Inner<T>>,
    to_read: AtomicBool,
}

pub struct Reader<T> {
    inner: Arc<Shared<T>>,
}

pub struct Writer<T> {
    inner: Arc<Shared<T>>,
}

impl<T> Inner<T>
where
    T: Clone,
{
    fn new(init: T) -> Self {
        Inner { data: init }
    }
}

impl<T> Shared<T>
where
    T: Clone,
{
    fn new(init: T) -> Self {
        Shared {
            inner: Mutex::new(Inner::new(init)),
            to_read: AtomicBool::new(false),
        }
    }

    fn write(&self, value: T) {
        let mut inner = self.inner.lock().unwrap();
        inner.data = value;
        self.to_read.store(true, Ordering::Release);
    }

    fn read(&self, value: &mut T) -> bool {
        if self.to_read.load(Ordering::Acquire) {
            let inner = self.inner.lock().unwrap();
            *value = inner.data.clone();
            self.to_read.store(false, Ordering::Release);
            true
        } else {
            false
        }
    }
}

impl<T> Reader<T>
where
    T: Clone,
{
    pub fn read(&self, value: &mut T) -> bool {
        self.inner.read(value)
    }
}

impl<T> Writer<T>
where
    T: Clone,
{
    pub fn write(&self, value: T) {
        self.inner.write(value)
    }
}

pub fn new<T>(init: T) -> (Reader<T>, Writer<T>)
where
    T: Clone,
{
    let inner = Arc::new(Shared::new(init));
    let r = Reader {
        inner: Arc::clone(&inner),
    };
    let w = Writer {
        inner: Arc::clone(&inner),
    };
    (r, w)
}
