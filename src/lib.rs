pub trait Reader {
    type Item;
    fn read(&self) -> Option<Self::Item>;
}

pub trait Writer {
    type Item;
    fn write(&self, value: Self::Item);
}

pub mod atomic_spsc;
pub mod blocking_spsc;
pub mod mutex_spsc;
pub mod ticket_spsc;
