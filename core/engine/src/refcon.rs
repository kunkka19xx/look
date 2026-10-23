//! The reference connector: every frame Look speaks, in one small program.
//!
//! It is the worked example `docs/integrating.md` points at, and the fixture
//! the connector integration tests spawn. A binary rather than a shell script
//! because the tests have to run on Windows too.
//!
//! ```text
//! refcon <block-id> [mode]
//! ```
//!
//! Modes: `serve` (the default, and the only one worth copying), `once` to
//! exit after its rows, `same` to send the same rows twice, and `oversized`
//! to breach the frame cap. The last three exist for the tests.

use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let block = args
        .first()
        .map_or("refcon.notes", String::as_str)
        .to_string();
    let mode = args.get(1).map_or("serve", String::as_str);

    // Look opens with a hello. Read it before answering, so a connector that
    // is started and never used costs nothing.
    let mut hello = String::new();
    if std::io::stdin().read_line(&mut hello).unwrap_or(0) == 0 {
        return;
    }

    say(&serde_json::json!({
        "op": "ready",
        "capabilities": ["rows", "preview", "plan"],
    }));

    match mode {
        // Exits with its rows already sent, which is what a crash looks like
        // from here. The pid makes each run's rows distinct, so the restart is
        // visible: identical rows would be deduped, correctly.
        "once" => {
            say(&serde_json::json!({
                "op": "rows",
                "block": block,
                "rows": [{ "id": "a", "title": format!("Alpha {}", std::process::id()) }],
            }));
            return;
        }
        // The same rows again are not news, and cost nothing past the compare.
        "same" => {
            rows(&block);
            std::thread::sleep(std::time::Duration::from_millis(200));
            rows(&block);
        }
        // The two sides of the frame cap. Nothing valid is sent first in
        // either: the point is exactly which one lands.
        "atcap" => say(&sized(&block, look_engine::connectors::MAX_FRAME_BYTES)),
        "oversized" => say(&sized(&block, look_engine::connectors::MAX_FRAME_BYTES + 1)),
        _ => rows(&block),
    }

    serve(&block);
}

/// The two rows this connector has. A real one would push again whenever its
/// data changed, which is the whole point of staying alive.
fn rows(block: &str) {
    say(&serde_json::json!({
        "op": "rows",
        "block": block,
        "rows": [
            { "id": "n1", "title": "Alpha", "subtitle": "the first row" },
            { "id": "n2", "title": "Beta", "subtitle": "the second row" },
        ],
    }));
}

/// A rows frame whose line is exactly `bytes` long, before the newline.
fn sized(block: &str, bytes: usize) -> serde_json::Value {
    let mut frame = serde_json::json!({
        "op": "rows",
        "block": block,
        "rows": [{ "id": "big", "title": "" }],
    });
    let envelope = frame.to_string().len();
    frame["rows"][0]["title"] = serde_json::Value::String("x".repeat(bytes - envelope));
    frame
}

/// Answers asks until Look says goodbye or closes the pipe.
fn serve(block: &str) {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(ask) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        match ask.get("op").and_then(serde_json::Value::as_str) {
            Some("bye") => return,
            Some("preview") => preview(block, &ask),
            Some("plan") => plan(block, &ask),
            Some("run") => run(block, &ask),
            // An op this program does not know is ignored, never fatal. Look
            // holds itself to the same rule.
            _ => {}
        }
    }
}

fn preview(block: &str, ask: &serde_json::Value) {
    let id = ask
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let text = match id {
        "n1" => "# Alpha\n\nThe **first** row, written in `markdown`.",
        "n2" => "# Beta\n\nThe second row.\n\n- one\n- two",
        other => {
            say(&serde_json::json!({
                "op": "error",
                "message": format!("no row {other}"),
            }));
            return;
        }
    };
    say(&serde_json::json!({
        "op": "preview",
        "block": block,
        "id": id,
        // Echoed back, or Look cannot tell this answer from the one before it.
        "req": ask.get("req"),
        "content_type": "markdown",
        "text": text,
    }));
}

/// What the verb would do, named row by row. This is the confirmation the
/// user reads, which is why it beats a declared `confirm` string: that string
/// cannot know how many rows were picked or what they are called.
fn plan(block: &str, ask: &serde_json::Value) {
    let ids: Vec<&str> = ask
        .get("ids")
        .and_then(serde_json::Value::as_array)
        .map(|ids| ids.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    let verb = ask
        .get("verb")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("do");
    say(&serde_json::json!({
        "op": "planned",
        "block": block,
        // Echoed back, or Look cannot tell this plan from the one before it.
        "plan": ask.get("plan"),
        "title": format!("{verb} {} row(s)", ids.len()),
        "detail": ids.join(", "),
    }));
}

fn run(block: &str, ask: &serde_json::Value) {
    say(&serde_json::json!({
        "op": "result",
        "block": block,
        "plan": ask.get("plan"),
        "ok": true,
        "summary": "nothing really happened, this is the reference connector",
    }));
}

/// One JSON object per line, flushed: Look reads line by line, and a buffered
/// frame is a frame that never arrives.
fn say(frame: &serde_json::Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{frame}");
    let _ = out.flush();
}
