use std::fs::OpenOptions;
use std::hint::black_box;
use std::io::prelude::*;
use std::marker::Send;
use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use rustedrazors::{atomic_spsc, blocking_spsc, mutex_spsc, ticket_spsc};
use rustedrazors::{Reader, Writer};

const PAYLOAD_SIZE: usize = 1024;
const ITERS: usize = 1000000;

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

fn write_ops<R, W>(r: R, w: W) -> Vec<u128>
where
    R: Reader<Payload> + Send,
    W: Writer<Payload> + Send,
{
    let barrier = Arc::new(Barrier::new(2));
    let (tx, rx) = mpsc::channel();

    let res = thread::scope(|s| {
        let r_handle = s.spawn({
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                for i in 0usize.. {
                    black_box(r.read());
                    if i % 64 == 0 && rx.try_recv().is_ok() {
                        break;
                    }
                }
            }
        });
        let w_handle = s.spawn({
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                let value = Payload::default();
                let mut success = Vec::with_capacity(ITERS);
                for _ in 0..ITERS {
                    let start = Instant::now();
                    black_box(w.write(black_box(value)));
                    let ns = start.elapsed().as_nanos();
                    success.push(ns);
                }
                _ = tx.send(());
                success
            }
        });

        (r_handle.join(), w_handle.join())
    });

    match res {
        (Ok(_), Ok(nanos)) => nanos,
        _ => {
            panic!("Something went wrong");
        }
    }
}

fn read_ops<R, W>(r: R, w: W) -> (Vec<u128>, Vec<u128>)
where
    R: Reader<Payload> + Send,
    W: Writer<Payload> + Send,
{
    let barrier = Arc::new(Barrier::new(2));
    let (tx, rx) = mpsc::channel();

    let res = thread::scope(|s| {
        let r_handle = s.spawn({
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                let mut success = Vec::with_capacity(ITERS);
                let mut failure = Vec::with_capacity(ITERS);
                for _ in 0..ITERS {
                    let start = Instant::now();
                    let res = black_box(r.read());
                    let ns = start.elapsed().as_nanos();
                    if res.is_some() {
                        success.push(ns);
                    } else {
                        failure.push(ns);
                    }
                }
                _ = tx.send(());
                (success, failure)
            }
        });
        let w_handle = s.spawn({
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                let value = Payload::default();
                for i in 0usize.. {
                    black_box(w.write(black_box(value)));
                    if i % 64 == 0 && rx.try_recv().is_ok() {
                        break;
                    }
                }
            }
        });

        (r_handle.join(), w_handle.join())
    });

    match res {
        (Ok(nanos), Ok(_)) => nanos,
        _ => {
            panic!("Something went wrong");
        }
    }
}

fn bench_function_impl(
    name: &str,
    read_fun: fn() -> (Vec<u128>, Vec<u128>),
    write_fun: fn() -> Vec<u128>,
) {
    let (success, failure) = read_fun();
    let writes = write_fun();

    let open = |filename| {
        OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(filename)
            .expect("Unable to create file")
    };

    let filename = format!("{}_success.txt", name);
    let mut f = open(filename);
    for i in success {
        _ = writeln!(f, "{}", i);
    }

    let filename = format!("{}_failure.txt", name);
    let mut f = open(filename);
    for i in failure {
        _ = writeln!(f, "{}", i);
    }

    let filename = format!("{}_writes.txt", name);
    let mut f = open(filename);
    for i in writes {
        _ = writeln!(f, "{}", i);
    }
}

macro_rules! bench_function {
    ($name:expr, $factory:ident) => {{
        bench_function_impl(
            $name,
            || {
                let (r, w) = $factory::new::<Payload>(Payload::default());
                read_ops(r, w)
            },
            || {
                let (r, w) = $factory::new::<Payload>(Payload::default());
                write_ops(r, w)
            },
        );
    }};
}

fn main() {
    bench_function!("atomic_reader", atomic_spsc);
    bench_function!("blocking_reader", blocking_spsc);
    bench_function!("mutex_reader", mutex_spsc);
    bench_function!("ticket_reader", ticket_spsc);
}
