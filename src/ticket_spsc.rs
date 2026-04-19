use crate::{ReadGuard, ReadState, Reader, Writer};

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
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

struct Inner<T> {
    data: TicketMutex<(T, ReadState)>,
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
            data: TicketMutex::new((init, ReadState::Stale)),
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

pub struct Guard<'a, T> {
    guard: TicketGuard<'a, (T, ReadState)>,
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard.0
    }
}

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

impl<T> std::fmt::Debug for Guard<'_, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
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
