use look_engine::QueryEngine;
use look_matching::{fuzzy_score, fuzzy_score_prepared, prepare_query};
use look_storage::SqliteStore;
use std::env;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct QueryBenchStats {
    query: &'static str,
    iterations: usize,
    p50_us: u128,
    p95_us: u128,
    avg_us: u128,
    min_us: u128,
    max_us: u128,
}

struct FuzzyBenchStats {
    mode: &'static str,
    scenario: &'static str,
    operations_per_iteration: usize,
    iterations: usize,
    p50_ns: u128,
    p95_ns: u128,
    avg_ns: u128,
    avg_per_op_ns: u128,
    min_ns: u128,
    max_ns: u128,
}

fn main() {
    let db_path = default_db_path();
    let started = Instant::now();
    if let Err(err) = QueryEngine::bootstrap_sqlite(&db_path) {
        eprintln!("index bootstrap failed: {err}");
        std::process::exit(1);
    }
    let index_elapsed = started.elapsed();

    let candidate_count =
        match SqliteStore::open(&db_path).and_then(|store| store.load_candidates(None)) {
            Ok(candidates) => candidates.len(),
            Err(err) => {
                eprintln!("failed to count candidates: {err}");
                std::process::exit(1);
            }
        };

    let engine = match QueryEngine::from_sqlite(&db_path) {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("failed to initialize engine: {err}");
            std::process::exit(1);
        }
    };

    let query_cases = [
        "",
        "sa",
        "net",
        "doc",
        "visual",
        "privacy security",
        "a\"safari",
        "f\"note",
        "d\"down",
        "r\"^visual.*",
    ];

    let mut query_stats = Vec::new();
    for query in query_cases {
        query_stats.push(bench_query(&engine, query, 40, 300));
    }

    let fuzzy_stats = [
        bench_fuzzy_raw(
            "launcher_like_short_queries",
            &["s", "sa", "saf", "vsc", "chr", "dock", "set", "blu", "priv"],
            &[
                "safari",
                "visual studio code",
                "google chrome",
                "system settings",
                "activity monitor",
                "bluetooth file exchange",
                "finder",
                "notes",
                "downloads",
                "documents",
            ],
            400,
        ),
        bench_fuzzy_raw(
            "gap_and_boundary_patterns",
            &["vsc", "gc", "sm", "net", "dwn"],
            &[
                "vs code",
                "visual studio code",
                "google chrome",
                "screen mirror",
                "network utility",
                "my downloads folder",
                "a x x x x c",
                "ab x c",
            ],
            600,
        ),
        bench_fuzzy_prepared_queries(
            "launcher_like_short_queries",
            &["s", "sa", "saf", "vsc", "chr", "dock", "set", "blu", "priv"],
            &[
                "safari",
                "visual studio code",
                "google chrome",
                "system settings",
                "activity monitor",
                "bluetooth file exchange",
                "finder",
                "notes",
                "downloads",
                "documents",
            ],
            400,
        ),
        bench_fuzzy_prepared_queries(
            "gap_and_boundary_patterns",
            &["vsc", "gc", "sm", "net", "dwn"],
            &[
                "vs code",
                "visual studio code",
                "google chrome",
                "screen mirror",
                "network utility",
                "my downloads folder",
                "a x x x x c",
                "ab x c",
            ],
            600,
        ),
    ];

    println!("# look benchmark");
    println!("db_path={}", db_path.display());
    println!("candidate_count={candidate_count}");
    println!(
        "index_elapsed_ms={} index_throughput_per_sec={:.2}",
        index_elapsed.as_millis(),
        throughput_per_second(candidate_count, index_elapsed)
    );
    println!("query,iterations,p50_us,p95_us,avg_us,min_us,max_us");
    for stat in query_stats {
        println!(
            "{},{},{},{},{},{},{}",
            stat.query,
            stat.iterations,
            stat.p50_us,
            stat.p95_us,
            stat.avg_us,
            stat.min_us,
            stat.max_us
        );
    }

    println!(
        "fuzzy_mode,fuzzy_scenario,ops_per_iter,iterations,p50_ns,p95_ns,avg_ns,avg_per_op_ns,min_ns,max_ns"
    );
    for stat in fuzzy_stats {
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            stat.mode,
            stat.scenario,
            stat.operations_per_iteration,
            stat.iterations,
            stat.p50_ns,
            stat.p95_ns,
            stat.avg_ns,
            stat.avg_per_op_ns,
            stat.min_ns,
            stat.max_ns
        );
    }
}

fn bench_query(
    engine: &QueryEngine,
    query: &'static str,
    limit: usize,
    iterations: usize,
) -> QueryBenchStats {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let results = engine.search(query, limit);
        black_box(results.len());
        samples.push(started.elapsed().as_micros());
    }
    samples.sort_unstable();

    let p50_us = percentile(&samples, 50);
    let p95_us = percentile(&samples, 95);
    let min_us = *samples.first().unwrap_or(&0);
    let max_us = *samples.last().unwrap_or(&0);
    let total_us: u128 = samples.iter().copied().sum();
    let avg_us = if samples.is_empty() {
        0
    } else {
        total_us / samples.len() as u128
    };

    QueryBenchStats {
        query,
        iterations,
        p50_us,
        p95_us,
        avg_us,
        min_us,
        max_us,
    }
}

fn bench_fuzzy_raw(
    scenario: &'static str,
    queries: &[&str],
    titles: &[&str],
    iterations: usize,
) -> FuzzyBenchStats {
    let operations_per_iteration = queries.len() * titles.len();
    let mut samples = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let started = Instant::now();
        let mut hit_count = 0usize;
        for query in queries {
            for title in titles {
                if fuzzy_score(query, title).is_some() {
                    hit_count += 1;
                }
            }
        }
        black_box(hit_count);
        samples.push(started.elapsed().as_nanos());
    }

    samples.sort_unstable();
    let p50_ns = percentile(&samples, 50);
    let p95_ns = percentile(&samples, 95);
    let min_ns = *samples.first().unwrap_or(&0);
    let max_ns = *samples.last().unwrap_or(&0);
    let total_ns: u128 = samples.iter().copied().sum();
    let avg_ns = if samples.is_empty() {
        0
    } else {
        total_ns / samples.len() as u128
    };
    let avg_per_op_ns = if operations_per_iteration == 0 {
        0
    } else {
        avg_ns / operations_per_iteration as u128
    };

    FuzzyBenchStats {
        mode: "raw",
        scenario,
        operations_per_iteration,
        iterations,
        p50_ns,
        p95_ns,
        avg_ns,
        avg_per_op_ns,
        min_ns,
        max_ns,
    }
}

fn bench_fuzzy_prepared_queries(
    scenario: &'static str,
    queries: &[&str],
    titles: &[&str],
    iterations: usize,
) -> FuzzyBenchStats {
    let prepared_queries: Vec<_> = queries.iter().map(|query| prepare_query(query)).collect();
    let operations_per_iteration = prepared_queries.len() * titles.len();
    let mut samples = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let started = Instant::now();
        let mut hit_count = 0usize;
        for query in &prepared_queries {
            for title in titles {
                if fuzzy_score_prepared(query, title).is_some() {
                    hit_count += 1;
                }
            }
        }
        black_box(hit_count);
        samples.push(started.elapsed().as_nanos());
    }

    samples.sort_unstable();
    let p50_ns = percentile(&samples, 50);
    let p95_ns = percentile(&samples, 95);
    let min_ns = *samples.first().unwrap_or(&0);
    let max_ns = *samples.last().unwrap_or(&0);
    let total_ns: u128 = samples.iter().copied().sum();
    let avg_ns = if samples.is_empty() {
        0
    } else {
        total_ns / samples.len() as u128
    };
    let avg_per_op_ns = if operations_per_iteration == 0 {
        0
    } else {
        avg_ns / operations_per_iteration as u128
    };

    FuzzyBenchStats {
        mode: "prepared_query",
        scenario,
        operations_per_iteration,
        iterations,
        p50_ns,
        p95_ns,
        avg_ns,
        avg_per_op_ns,
        min_ns,
        max_ns,
    }
}

fn percentile(samples: &[u128], p: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let rank = ((samples.len() - 1) * p) / 100;
    samples[rank]
}

fn throughput_per_second(count: usize, duration: Duration) -> f64 {
    let secs = duration.as_secs_f64();
    if secs <= f64::EPSILON {
        return 0.0;
    }
    count as f64 / secs
}

fn default_db_path() -> PathBuf {
    if let Ok(custom) = env::var("LOOK_DB_PATH")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("look")
        .join("look.db")
}
