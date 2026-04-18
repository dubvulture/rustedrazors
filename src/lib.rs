pub trait Reader {
    /// Underlying item we are reading
    type Item;
    /// MutexGuard-like handle to be returned
    type Guard<'a>: std::ops::Deref<Target = Self::Item>
    where
        Self: 'a;

    fn read(&self) -> Option<Self::Guard<'_>>;
}

pub trait Writer {
    /// Underlying item we are writing
    type Item;

    fn write(&self, value: Self::Item);
}

pub mod atomic_spsc;
pub mod blocking_spsc;
pub mod mutex_spsc;
pub mod ticket_spsc;
