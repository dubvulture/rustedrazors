use std::ops::Deref;

#[derive(Copy, Clone, PartialEq)]
pub enum ReadState {
    /// value changed since last read
    Fresh,
    /// value is the same as last read
    Stale,
}

pub trait ReadGuard<'a, T>: Deref<Target = T> + 'a {
    fn state(&self) -> ReadState;

    fn is_fresh(&self) -> bool {
        self.state() == ReadState::Fresh
    }

    fn is_stale(&self) -> bool {
        self.state() == ReadState::Stale
    }
}

pub trait Reader {
    /// Underlying item we are reading
    type Item;
    /// MutexGuard-like handle to be returned
    type Guard<'a>: ReadGuard<'a, Self::Item>
    where
        Self: 'a;

    fn read(&self) -> Self::Guard<'_>;
}

pub trait Writer {
    /// Underlying item we are writing
    type Item;

    fn write(&self, value: Self::Item);
}

pub mod atomic_spsc;
pub mod mutex_spsc;
pub mod ticket_spsc;
