//! What one `rows` push costs Look, which is the figure `docs/integrating.md`
//! quotes when it tells a connector author to push whenever their data
//! changes.
//!
//! The interesting case is the unchanged push: a connector watching something
//! that has not moved sends the same rows over and over, and that is the state
//! it spends most of its life in. Look used to answer it by reading the whole
//! cache file back; it now hashes what it last wrote. Both are timed here
//! against the same payload, so the note can quote a ratio rather than a
//! promise.
//!
//! ```bash
//! cargo run --release --manifest-path core/Cargo.toml --example connector_push
//! ```

use std::hash::{Hash, Hasher};
use std::time::Instant;

/// A full block: the cap a connector's rows are held to.
const ROWS: usize = 2_000;
const PUSHES: usize = 1_000;
const BLOCK: &str = "bench.rows";

fn main() {
    let dir = std::env::temp_dir().join(format!("look-connector-push-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("cache dir");
    unsafe { std::env::set_var(look_engine::index::ROWS_CACHE_DIR_ENV, &dir) };

    let text = payload();
    look_engine::index::store_counted_rows(BLOCK, &text, ROWS).expect("seed the cache");
    println!(
        "rows: {ROWS}, payload: {} KiB, pushes: {PUSHES}",
        text.len() / 1024
    );

    // What an unchanged push used to cost: the whole file, read and compared.
    let began = Instant::now();
    for _ in 0..PUSHES {
        assert!(look_engine::index::run_cache_holds(BLOCK, &text));
    }
    let reading = began.elapsed();

    // What it costs now.
    let began = Instant::now();
    for _ in 0..PUSHES {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        std::hint::black_box(hasher.finish());
    }
    let hashing = began.elapsed();

    let each = |total: std::time::Duration| total.as_secs_f64() * 1_000.0 / PUSHES as f64;
    println!("read and compare: {:.4} ms per push", each(reading));
    println!("hash:             {:.4} ms per push", each(hashing));
    println!(
        "ratio:            {:.1}x cheaper",
        reading.as_secs_f64() / hashing.as_secs_f64()
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The same shape `connectors::store` builds, so the bytes are the bytes.
fn payload() -> String {
    let rows: Vec<serde_json::Value> = (0..ROWS)
        .map(|row| {
            serde_json::json!({
                "id": format!("row-{row}"),
                "title": format!("Row number {row}"),
                "subtitle": "pushed by a connector watching something",
                "path": serde_json::Value::Null,
                "icon": serde_json::Value::Null,
            })
        })
        .collect();
    serde_json::to_string(&rows).expect("payload")
}
