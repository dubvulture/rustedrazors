pub trait Reader<T> {
    fn read(&self, value: &mut T) -> bool;
}

pub trait Writer<T> {
    fn write(&self, value: T);
}

pub mod atomic_spsc;
