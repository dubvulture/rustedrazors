pub trait Reader<T> {
    fn read(&self) -> Option<T>;
}

pub trait Writer<T> {
    fn write(&self, value: T);
}

pub mod atomic_spsc;
pub mod blocking_spsc;
pub mod mutex_spsc;
pub mod ticket_spsc;
