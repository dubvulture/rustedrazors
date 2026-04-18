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
        let mut i = 0;
        while self.now_serving.load(Ordering::Acquire) != ticket {
            i += 1;
            if i >= 20 {
                std::thread::yield_now();
            }
        }
        Ok(TicketGuard::new(&self))
    }

    fn unlock(&self) {
        let now_serving = self.now_serving.load(Ordering::Relaxed) + 1;
        self.now_serving.store(now_serving, Ordering::Release);
    }
}

pub struct TicketGuard<'a, T> {
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

impl<T> std::fmt::Debug for TicketGuard<'_, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
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

impl<T> Inner<T> {
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

    fn read(&self) -> Option<TicketGuard<'_, T>> {
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
        = TicketGuard<'a, T>
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
