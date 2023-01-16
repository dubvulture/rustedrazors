use std::hint::black_box;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

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

macro_rules! rw_ops {
    ($r:ident, $w:ident) => {{
        const RUNTIME: Duration = Duration::from_secs(5);

        let barrier = Arc::new(Barrier::new(2));

        let r_handle = thread::spawn({
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                let start = Instant::now();
                let mut value = Payload::default();
                let mut iters: u64 = 0;
                loop {
                    black_box(value);
                    $r.read(&mut value);
                    iters += 1;

                    let elapsed = Instant::now() - start;
                    if elapsed > RUNTIME {
                        break;
                    }
                }
                iters
            }
        });
        let w_handle = thread::spawn({
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                let start = Instant::now();
                let value = Payload::default();
                let mut iters: u64 = 0;
                loop {
                    black_box(value);
                    $w.write(value);
                    iters += 1;

                    let elapsed = Instant::now() - start;
                    if elapsed > RUNTIME {
                        break;
                    }
                }
                iters
            }
        });

        let r_res = r_handle.join();
        let w_res = w_handle.join();

        match (r_res, w_res) {
            (Ok(r_iters), Ok(w_iters)) => {
                let r_ops = r_iters as f64 / RUNTIME.as_secs() as f64 / 1000f64;
                let w_ops = w_iters as f64 / RUNTIME.as_secs() as f64 / 1000f64;
                (r_ops, w_ops)
            }
            _ => {
                panic!("Something went wrong");
            }
        }
    }};
}

fn bench_function(name: &'static str, fun: fn() -> (f64, f64)) {
    let (r, w) = fun();
    println!("{} | Reader {} Kops/s", name, r);
    println!("{} | Writer {} Kops/s", name, w);
}

fn main() {
    bench_function("mutex", || {
        let (r, w) = mutex_spsc::new::<Payload>(Payload::default());
        rw_ops!(r, w)
    });
    bench_function("atomic", || {
        let (r, w) = atomic_spsc::new::<Payload>(Payload::default());
        rw_ops!(r, w)
    });
}
