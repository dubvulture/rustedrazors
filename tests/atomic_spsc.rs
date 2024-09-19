#[cfg(test)]
mod tests {

    use std::thread;

    use rustedrazors::atomic_spsc;
    use rustedrazors::{Reader, Writer};

    #[derive(Clone)]
    struct ClonePayload {
        _p: [u8; 1024],
    }

    #[derive(Clone, Copy)]
    struct CopyPayload {
        _p: [u8; 1024],
    }

    impl Default for ClonePayload {
        fn default() -> Self {
            ClonePayload { _p: [0; 1024] }
        }
    }

    impl Default for CopyPayload {
        fn default() -> Self {
            CopyPayload { _p: [0; 1024] }
        }
    }

    #[test]
    fn test_compilation() {
        // This compiling is a success by itself
        // Allow creation and usage across threads of atomic_spsc with either clonable and copayable
        // types

        let (clone_r, clone_w) = atomic_spsc::new::<ClonePayload>(ClonePayload::default());

        let _ = thread::spawn(move || {
            let _ = clone_r.read();
        })
        .join();
        let _ = thread::spawn(move || {
            let clone_p = ClonePayload::default();
            clone_w.write(clone_p);
        })
        .join();

        let (copy_r, copy_w) = atomic_spsc::new::<CopyPayload>(CopyPayload::default());
        let _ = thread::spawn(move || {
            let _ = copy_r.read();
        })
        .join();
        let _ = thread::spawn(move || {
            let copy_p = CopyPayload::default();
            copy_w.write(copy_p);
        })
        .join();
    }

    #[test]
    fn test_basics() {
        // Test basic API

        let (r, w) = atomic_spsc::new::<i32>(0);

        for _ in 0..5 {
            let res = r.read();
            assert_eq!(res, None, "Read should have failed");
        }

        w.write(22);

        let res = r.read();
        assert_eq!(
            res,
            Some(22),
            "Read should have returned the value previously written"
        );

        let res = r.read();
        assert_eq!(res, None, "Read should have failed");

        w.write(42);
        w.write(62);

        let res = r.read();
        assert_eq!(
            res,
            Some(62),
            "Read should have returned the value previously written"
        );
    }

    #[test]
    fn test_threading() {
        // Test atomic_spsc with i32 across threads with multiple iterations.
        // Maybe find a way to enable thread sanitizers?

        let (r, w) = atomic_spsc::new::<i32>(0);

        let read_res = thread::spawn(move || {
            for _ in 0..1000 {
                let _ = r.read();
            }
        })
        .join();
        assert!(
            read_res.is_ok(),
            "Reader thread should have ended peacefully"
        );
        let write_res = thread::spawn(move || {
            for i in 0..1000 {
                w.write(i);
            }
        })
        .join();
        assert!(
            write_res.is_ok(),
            "Writer thread should have ended peacefully"
        );
    }
}
