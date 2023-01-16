#![feature(test)]

extern crate test;

#[cfg(test)]
mod bench_ops {

    use std::thread;
    use std::time::{Duration, Instant};
    use test::Bencher;

    use rustedrazors::atomic_spsc;

    #[derive(Clone, Copy)]
    struct Payload {
        _p: [u8; 1024],
    }

    impl Default for Payload {
        fn default() -> Self {
            Payload { _p: [0; 1024] }
        }
    }

    #[bench]
    fn rw_ops(b: &mut Bencher) {
        b.iter(|| {
            let (mut r, mut w) = atomic_spsc::new::<Payload>(Payload::default());

            const RUNTIME: Duration = Duration::from_secs(1);

            let r_handle = thread::spawn(move || {
                let start = Instant::now();
                let mut value = Payload::default();
                let mut iters: u64 = 0;
                loop {
                    std::hint::black_box(value);
                    r.read(&mut value);
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
                    std::hint::black_box(value);
                    w.write(value);
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
                    println!(
                        "Reader {} Kops/s",
                        r_iters as f64 / RUNTIME.as_secs() as f64 / 1000f64
                    );
                    println!(
                        "Writer {} Kops/s",
                        w_iters as f64 / RUNTIME.as_secs() as f64 / 1000f64
                    );
                }
                _ => {
                    println!("Something went wrong");
                }
            }
        });
    }
}
