use std::hint::black_box;
use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use rustedrazors::atomic_spsc;
use rustedrazors::{Reader, Writer};
mod mutex_spsc;

const PAYLOAD_SIZE: usize = 1024;

#[derive(Clone, Copy)]
struct Payload {
    _p: [u8; PAYLOAD_SIZE],
}

impl Default for Payload {
    fn default() -> Self {
        Payload {
            _p: [0; PAYLOAD_SIZE],
        }
    }
}

fn read_ops<R, W>(r: R, w: W) -> (Vec<u128>, Vec<u128>)
where
    R: Reader<Payload> + std::marker::Send + 'static,
    W: Writer<Payload> + std::marker::Send + 'static,
{
    const ITERS: usize = 1000000;

    let barrier = Arc::new(Barrier::new(2));
    let (tx, rx) = mpsc::channel();

    let r_handle = thread::spawn({
        let barrier = Arc::clone(&barrier);
        move || {
            barrier.wait();
            let mut value = Payload::default();
            let mut success = Vec::with_capacity(ITERS);
            let mut failure = Vec::with_capacity(ITERS);
            for _ in 0..ITERS {
                let start = Instant::now();
                black_box(value);
                let res = r.read(&mut value);
                let ns = start.elapsed().as_nanos();
                if res {
                    success.push(ns);
                } else {
                    failure.push(ns);
                }
            }
            _ = tx.send(());
            (success, failure)
        }
    });
    let w_handle = thread::spawn({
        let barrier = Arc::clone(&barrier);
        move || {
            barrier.wait();
            let value = Payload::default();
            for i in 0.. {
                black_box(value);
                w.write(value);
                if i % 64 == 0 && rx.try_recv().is_ok() {
                    break;
                }
            }
        }
    });

    let r_res = r_handle.join();
    let w_res = w_handle.join();

    match (r_res, w_res) {
        (Ok(nanos), Ok(_)) => nanos,
        _ => {
            panic!("Something went wrong");
        }
    }
}

fn bench_function(name: &str, fun: fn() -> (Vec<u128>, Vec<u128>)) {
    let (success, failure) = fun();

    use std::fs::File;
    use std::io::{BufWriter, Write};

    let filename = format!("{}_success.txt", name);
    let mut f = BufWriter::new(File::create(filename).expect("Unable to create file"));
    for i in success {
        _ = write!(f, "{0}\n", i);
    }

    let filename = format!("{}_failure.txt", name);
    let mut f = BufWriter::new(File::create(filename).expect("Unable to create file"));
    for i in failure {
        _ = write!(f, "{0}\n", i);
    }
}

fn main() {
    bench_function("mutex_reader", || {
        let (r, w) = mutex_spsc::new::<Payload>(Payload::default());
        read_ops(r, w)
    });
    bench_function("atomic_reader", || {
        let (r, w) = atomic_spsc::new::<Payload>(Payload::default());
        read_ops(r, w)
    });
}
