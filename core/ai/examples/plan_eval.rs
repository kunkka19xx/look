//! Planner eval: runs a fixture corpus through the real routing ladder
//! (`route::route_json`) and, for whatever reaches the model, the real planner
//! request, then scores route / tool / params and reports latency. A live
//! Ollama is required unless `--routes-only`.
//!
//! ```text
//! cargo run -p look-ai --example plan_eval
//! cargo run -p look-ai --example plan_eval -- --model qwen3.5:9b --min 90
//! cargo run -p look-ai --example plan_eval -- --routes-only
//! ```
//!
//! Host and model default to `~/.look.config`, so a bare run scores exactly
//! what the app ships to this machine. Fixtures state DESIRED behaviour, so a
//! failure is a real gap whether it lives in the prompt or in the ladder.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use look_ai::{ollama, plan, planner, route};
use serde_json::Value;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/planner_eval.jsonl");
const DEFAULT_HOST: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen3.5:4b";

struct Case {
    input: String,
    route: String,
    /// Present when the case asserts tools. Empty = "must decline"; more than
    /// one = a compound request that must plan every step, in order.
    tools: Option<Vec<String>>,
    params: Value,
    note: Option<String>,
}

struct Failure {
    input: String,
    reason: String,
    note: Option<String>,
}

#[derive(Default)]
struct Tally {
    hit: usize,
    total: usize,
}

impl Tally {
    fn add(&mut self, ok: bool) {
        self.total += 1;
        self.hit += usize::from(ok);
    }

    fn pct(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            100.0 * self.hit as f64 / self.total as f64
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let routes_only = args.iter().any(|a| a == "--routes-only");
    let config = read_config();
    let host = flag("--host")
        .or_else(|| config.get("ollama_host").cloned())
        .unwrap_or_else(|| DEFAULT_HOST.into());
    let model = flag("--model")
        .or_else(|| config.get("ollama_model").cloned())
        .unwrap_or_else(|| DEFAULT_MODEL.into());
    let path = flag("--fixtures").unwrap_or_else(|| FIXTURES.into());
    let filter = flag("--filter");
    let min: Option<f64> = flag("--min").and_then(|v| v.parse().ok());

    let mut cases = load_cases(&path);
    if let Some(needle) = &filter {
        let needle = needle.to_lowercase();
        cases.retain(|c| {
            c.input.to_lowercase().contains(&needle)
                || c.tools
                    .as_ref()
                    .is_some_and(|tools| tools.iter().any(|t| t.contains(&needle)))
        });
    }
    if cases.is_empty() {
        eprintln!("no cases in {path}");
        std::process::exit(2);
    }

    // The memory tier EXECUTES (it is the handler, not a preview), so it gets a
    // throwaway store instead of the user's real one.
    let memory_path = std::env::temp_dir().join("look-plan-eval-memory.json");
    let _ = std::fs::remove_file(&memory_path);

    if !routes_only {
        planner::warm(&host, &model);
    }

    let now = chrono::Utc::now().timestamp();
    let (mut routes, mut tools, mut params) =
        (Tally::default(), Tally::default(), Tally::default());
    let mut by_tool: BTreeMap<String, (Tally, Tally)> = BTreeMap::new();
    let mut by_route: BTreeMap<String, Tally> = BTreeMap::new();
    let mut failures: Vec<Failure> = Vec::new();
    let mut latencies: Vec<u128> = Vec::new();

    for case in &cases {
        let decision: Value =
            serde_json::from_str(&route::route_json(&memory_path, &case.input, true, now))
                .unwrap_or(Value::Null);
        let actual_route = decision["route"].as_str().unwrap_or("?").to_string();
        let route_ok = actual_route == case.route;
        routes.add(route_ok);
        by_route
            .entry(case.route.clone())
            .or_default()
            .add(route_ok);
        if !route_ok {
            failures.push(Failure {
                input: case.input.clone(),
                reason: format!("route  {} -> {actual_route}", case.route),
                note: case.note.clone(),
            });
            continue;
        }

        let Some(expected_tools) = &case.tools else {
            print!(".");
            flush();
            continue;
        };
        let calls = match actual_route.as_str() {
            "explicit" => vec![decision["call"].clone()],
            "plan" if routes_only => continue,
            "plan" => {
                let started = Instant::now();
                let calls = plan_once(&host, &model, &case.input);
                latencies.push(started.elapsed().as_millis());
                calls
            }
            _ => Vec::new(),
        };

        let actual_tools: Vec<String> = calls
            .iter()
            .filter_map(|c| c["tool"].as_str().map(str::to_string))
            .collect();
        let tool_ok = actual_tools == *expected_tools;
        tools.add(tool_ok);
        let entry = by_tool.entry(label(expected_tools)).or_default();
        entry.0.add(tool_ok);
        if !tool_ok {
            failures.push(Failure {
                input: case.input.clone(),
                reason: format!(
                    "tool   {} -> {}",
                    label(expected_tools),
                    label(&actual_tools)
                ),
                note: case.note.clone(),
            });
            print!("x");
            flush();
            continue;
        }

        if let Some(expected) = case.params.as_object() {
            // Params are asserted against the first step; a compound case
            // asserts its shape through the tool list.
            let call = calls.first().cloned().unwrap_or(Value::Null);
            for (key, want) in expected {
                let got = call["params"][key].as_str().unwrap_or("");
                let ok = matches_param(want.as_str().unwrap_or(""), got);
                params.add(ok);
                entry.1.add(ok);
                if !ok {
                    failures.push(Failure {
                        input: case.input.clone(),
                        reason: format!(
                            "param  {key}: {} -> {}",
                            want.as_str().unwrap_or(""),
                            if got.is_empty() { "(missing)" } else { got }
                        ),
                        note: case.note.clone(),
                    });
                }
            }
        }
        print!(".");
        flush();
    }
    let _ = std::fs::remove_file(&memory_path);
    println!("\n");

    let model_cases = cases.iter().filter(|c| c.route == "plan").count();
    println!(
        "{model}   {} cases ({model_cases} model, {} deterministic)\n",
        cases.len(),
        cases.len() - model_cases
    );
    line("route", &routes);
    line("tool", &tools);
    line("params", &params);

    println!("\nby tool");
    for (tool, (hits, param)) in &by_tool {
        println!(
            "  {tool:<22} {:>2}/{:<2} {:>5.0}%   params {}/{}",
            hits.hit,
            hits.total,
            hits.pct(),
            param.hit,
            param.total
        );
    }
    println!("\nby route");
    for (name, tally) in &by_route {
        println!(
            "  {name:<22} {:>2}/{:<2} {:>5.0}%",
            tally.hit,
            tally.total,
            tally.pct()
        );
    }

    if !latencies.is_empty() {
        latencies.sort_unstable();
        println!(
            "\nlatency  p50 {}ms  p95 {}ms  max {}ms",
            latencies[latencies.len() / 2],
            latencies[latencies.len() * 95 / 100],
            latencies[latencies.len() - 1]
        );
    }

    if !failures.is_empty() {
        println!("\nfailures ({})", failures.len());
        for f in &failures {
            println!("  \"{}\"\n     {}", f.input, f.reason);
            if let Some(note) = &f.note {
                println!("     note: {note}");
            }
        }
    }

    if let Some(min) = min
        && tools.pct() < min
    {
        eprintln!("\ntool accuracy {:.0}% below --min {min:.0}%", tools.pct());
        std::process::exit(1);
    }
}

/// One planning round trip on the same body the app sends, reduced to the
/// resolved `{tool, params}` call (null when the model declines or garbles).
fn plan_once(host: &str, model: &str, input: &str) -> Vec<Value> {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    ollama::post_json(&url, &planner::request_body(model, input), 30)
        .and_then(|body| plan::parse_chat_response(&body))
        .map(planner::resolve_steps)
        .unwrap_or_default()
}

/// `~needle` asserts a normalized substring (titles and instructions have many
/// defensible phrasings); anything else is normalized equality.
fn matches_param(want: &str, got: &str) -> bool {
    let norm = |s: &str| {
        s.to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim_end_matches(['.', '!'])
            .to_string()
    };
    match want.strip_prefix('~') {
        Some(needle) => norm(got).contains(&norm(needle)),
        None => norm(got) == norm(want),
    }
}

/// "(decline)" for nothing, "a + b" for a compound plan.
fn label(tools: &[String]) -> String {
    if tools.is_empty() {
        "(decline)".into()
    } else {
        tools.join(" + ")
    }
}

fn load_cases(path: &str) -> Vec<Case> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(2);
    });
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .enumerate()
        .map(|(i, line)| {
            let v: Value = serde_json::from_str(line).unwrap_or_else(|e| {
                eprintln!("{path} line {}: {e}", i + 1);
                std::process::exit(2);
            });
            Case {
                input: v["input"].as_str().unwrap_or_default().to_string(),
                route: v["route"].as_str().unwrap_or("plan").to_string(),
                tools: v
                    .get("tools")
                    .and_then(|t| serde_json::from_value::<Vec<String>>(t.clone()).ok())
                    // `"tool": "x"` / `"tool": null` is the one-step shorthand.
                    .or_else(|| {
                        v.get("tool")
                            .map(|t| t.as_str().map(|s| vec![s.to_string()]).unwrap_or_default())
                    }),
                params: v.get("params").cloned().unwrap_or(Value::Null),
                note: v.get("note").and_then(|n| n.as_str()).map(str::to_string),
            }
        })
        .collect()
}

fn read_config() -> BTreeMap<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = PathBuf::from(home).join(".look.config");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn line(label: &str, tally: &Tally) {
    println!(
        "{label:<8} {:>3}/{:<3} {:>5.0}%",
        tally.hit,
        tally.total,
        tally.pct()
    );
}

fn flush() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
}
