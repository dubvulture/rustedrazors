use std::hint::black_box;
use std::thread;
use std::time::{Duration, Instant};

use rustedrazors::atomic_spsc;
mod mutex_spsc;

#[derive(Clone, Copy)]
struct Payload {
    _p: [u8; 1024],
}

impl Default for Payload {
    fn default() -> Self {
        Payload { _p: [0; 1024] }
    }
}

macro_rules! rw_ops {
    ($r:ident, $w:ident) => {{
        const RUNTIME: Duration = Duration::from_secs(1);

        let r_handle = thread::spawn(move || {
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
        });
        let w_handle = thread::spawn(move || {
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
        let (mut r, mut w) = mutex_spsc::new::<Payload>(Payload::default());
        rw_ops!(r, w)
    });
    bench_function("atomic", || {
        let (mut r, mut w) = atomic_spsc::new::<Payload>(Payload::default());
        rw_ops!(r, w)
    });
}
