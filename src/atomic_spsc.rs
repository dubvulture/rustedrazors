use std::cell::UnsafeCell;

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

const POOL_SIZE: usize = 3;

struct Data<T> {
    pool: [UnsafeCell<T>; POOL_SIZE],
    free: [AtomicBool; POOL_SIZE],
    // either -1 or in [0, POOL_SIZE)
    buffer: AtomicIsize,
}

/// Safety: enable SYnc when T is Send to allow sharing UnsafeCell.
/// UnsafeCell is accessed without data races by design.
unsafe impl<T> Sync for Data<T> where T: Send {}

pub struct Reader<T> {
    atom: Arc<Data<T>>,
}

pub struct Writer<T> {
    atom: Arc<Data<T>>,
}

impl<T> Data<T>
where
    T: Clone,
{
    /// Constructs a new [`Data`] initialized with the provided value.
    fn new(init: T) -> Self {
        Data {
            pool: [(); POOL_SIZE].map(|_| UnsafeCell::new(init.clone())),
            free: [(); POOL_SIZE].map(|_| AtomicBool::new(true)),
            buffer: AtomicIsize::new(-1),
        }
    }

    /// Writes the provided value.
    ///
    /// This method is wait-free since there is always a spot in the pool where we can write to.
    fn write(&self, value: T) {
        let idx = self.acquire();
        self.write_to(idx, value);
        // Safety: this is fine, idx can only be in [0, POOL_SIZE)
        let buffer = self.buffer.swap(idx as isize, Ordering::AcqRel);
        if buffer != -1 {
            self.release(buffer as usize);
        }
    }

    fn write_to(&self, idx: usize, value: T) {
        let pool = self.pool[idx].get();
        unsafe {
            *pool = value;
        }
    }

    /// Try reading the last written value.
    /// The operation may fail if no new value was written since the last read.
    ///
    /// This method is wait-free.
    fn read(&self, value: &mut T) -> bool {
        let buffer = self.buffer.swap(-1, Ordering::AcqRel);
        match buffer {
            -1 => false,
            buffer => {
                // Safety: this is fine, idx can only be in [0, POOL_SIZE)
                let buffer = buffer as usize;
                self.read_from(buffer, value);
                self.release(buffer);
                true
            }
        }
    }

    fn read_from(&self, idx: usize, value: &mut T) {
        let pool = self.pool[idx].get();
        unsafe {
            *value = (*pool).clone();
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

impl<T> Reader<T>
where
    T: Clone,
{
    pub fn read(&mut self, value: &mut T) -> bool {
        self.atom.read(value)
    }
}

impl<T> Writer<T>
where
    T: Clone,
{
    pub fn write(&mut self, value: T) {
        self.atom.write(value)
    }
}

/// Construct a new read and write handle pair from an data structure initialzied with `init`.
pub fn new<T>(init: T) -> (Reader<T>, Writer<T>)
where
    T: Clone,
{
    let atom = Arc::new(Data::new(init));
    let r = Reader { atom: atom.clone() };
    let w = Writer { atom };
    (r, w)
}
