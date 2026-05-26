use look_ffi::{look_free_cstring, look_search_json, look_search_json_compact};
use std::ffi::{CStr, CString};
use std::hint::black_box;
use std::time::Instant;

struct BenchStats {
    label: &'static str,
    iterations: usize,
    p50_us: u128,
    p95_us: u128,
    avg_us: u128,
    avg_payload_bytes: usize,
}

fn main() {
    let query = CString::new("concurrency go").expect("query cstring");
    let limit = 40u32;
    let iterations = 300usize;

    let full = bench("full_json", iterations, || unsafe {
        let ptr = look_search_json(query.as_ptr(), limit);
        consume_and_free(ptr)
    });

    let compact = bench("compact_json", iterations, || unsafe {
        let ptr = look_search_json_compact(query.as_ptr(), limit);
        consume_and_free(ptr)
    });

    println!("# ffi search json benchmark");
    println!("query=concurrency go limit={limit} iterations={iterations}");
    println!("mode,iterations,p50_us,p95_us,avg_us,avg_payload_bytes");
    print_stats(&full);
    print_stats(&compact);
}

fn bench(label: &'static str, iterations: usize, mut f: impl FnMut() -> usize) -> BenchStats {
    let mut samples = Vec::with_capacity(iterations);
    let mut total_bytes = 0usize;
    for _ in 0..iterations {
        let started = Instant::now();
        let bytes = f();
        samples.push(started.elapsed().as_micros());
        total_bytes = total_bytes.saturating_add(bytes);
        black_box(bytes);
    }
    samples.sort_unstable();

    let p50_us = percentile(&samples, 50);
    let p95_us = percentile(&samples, 95);
    let total_us: u128 = samples.iter().copied().sum();
    let avg_us = if samples.is_empty() {
        0
    } else {
        total_us / samples.len() as u128
    };
    let avg_payload_bytes = if iterations == 0 {
        0
    } else {
        total_bytes / iterations
    };

    BenchStats {
        label,
        iterations,
        p50_us,
        p95_us,
        avg_us,
        avg_payload_bytes,
    }
}

unsafe fn consume_and_free(ptr: *mut i8) -> usize {
    if ptr.is_null() {
        return 0;
    }

    let raw = unsafe { CStr::from_ptr(ptr) }.to_bytes().len();
    look_free_cstring(ptr);
    raw
}

fn percentile(samples: &[u128], p: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }

    let rank = ((samples.len() - 1) * p) / 100;
    samples[rank]
}

fn print_stats(stats: &BenchStats) {
    println!(
        "{},{},{},{},{},{}",
        stats.label,
        stats.iterations,
        stats.p50_us,
        stats.p95_us,
        stats.avg_us,
        stats.avg_payload_bytes
    );
}
