/// Implement a trivial atomic_spsc-like data structures using a Mutex
use std::sync::{Arc, Mutex};

struct Inner<T> {
    data: T,
    to_read: bool,
}

struct Shared<T> {
    inner: Mutex<Inner<T>>,
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
        Inner {
            data: init,
            to_read: false,
        }
    }
}

impl<T> Shared<T>
where
    T: Clone,
{
    fn new(init: T) -> Self {
        Shared {
            inner: Mutex::new(Inner::new(init)),
        }
    }

    fn write(&self, value: T) {
        let mut inner = self.inner.lock().unwrap();
        inner.data = value;
        inner.to_read = true;
    }

    fn read(&self, value: &mut T) -> bool {
        let mut inner = self.inner.lock().unwrap();
        *value = inner.data.clone();
        inner.to_read = false;
        true
    }
}

impl<T> Reader<T>
where
    T: Clone,
{
    pub fn read(&mut self, value: &mut T) -> bool {
        self.inner.read(value)
    }
}

impl<T> Writer<T>
where
    T: Clone,
{
    pub fn write(&mut self, value: T) {
        self.inner.write(value)
    }
}

pub fn new<T>(init: T) -> (Reader<T>, Writer<T>)
where
    T: Clone,
{
    let inner = Arc::new(Shared::new(init));
    let r = Reader {
        inner: inner.clone(),
    };
    let w = Writer { inner };
    (r, w)
}
