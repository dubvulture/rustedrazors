use crate::{Reader, Writer};

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

struct TicketMutex<T> {
    data: UnsafeCell<T>,
    now_serving: AtomicU64,
    next_ticket: AtomicU64,
}

unsafe impl<T> Sync for TicketMutex<T> where T: Send {}

impl<T> TicketMutex<T> {
    fn new(init: T) -> Self {
        TicketMutex {
            data: UnsafeCell::new(init),
            now_serving: AtomicU64::new(0),
            next_ticket: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> Result<TicketGuard<'_, T>, ()> {
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        while self.now_serving.load(Ordering::Acquire) != ticket {
            // TODO: yield?
        }
        Ok(TicketGuard::new(&self))
    }

    fn unlock(&self) {
        let now_serving = self.now_serving.load(Ordering::Relaxed) + 1;
        self.now_serving.store(now_serving, Ordering::Release);
    }
}

struct TicketGuard<'a, T> {
    mutex: &'a TicketMutex<T>,
}

impl<'mutex, T> TicketGuard<'mutex, T> {
    fn new(mutex: &'mutex TicketMutex<T>) -> Self {
        TicketGuard { mutex }
    }
}

impl<T> Deref for TicketGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for TicketGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for TicketGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock()
    }
}

struct Inner<T> {
    data: TicketMutex<T>,
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
            data: TicketMutex::new(init),
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
