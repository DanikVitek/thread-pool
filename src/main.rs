use std::{sync::atomic::{self, AtomicUsize}, thread, time::{Duration, Instant}};

use thread_pool::ThreadPool;

fn main() {
    let pool = ThreadPool::default();

    static TOTAL: AtomicUsize = AtomicUsize::new(0);

    let start = Instant::now();
    for _ in 0..12 {
        let total = &TOTAL;
        pool.run(move || {
            thread::sleep(Duration::from_secs(1));
            total.fetch_add(1, atomic::Ordering::Relaxed);
        });
    }

    drop(pool.clone());
    drop(pool);

    println!("total: {}", TOTAL.load(atomic::Ordering::Relaxed));
    println!("Elapsed: {:?}", start.elapsed());
}
