//! Temporary probe: raw model output for one query under the sharded prompt
//! vs the full-vocabulary prompt.

use look_ai::{domain, ollama, planner};
use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let model = args.first().cloned().unwrap_or("qwen3.5:4b".into());
    let query = args
        .get(1)
        .cloned()
        .unwrap_or("remind me to buy milk".into());
    let url = "http://localhost:11434/api/chat";

    for (label, domain) in [("shard", domain::of(&query)), ("full", None)] {
        let prompt = planner::system_prompt(domain, &query);
        let mut aliases: Vec<&str> = if domain.is_some() {
            vec!["complete", "delete", "reminder", "snooze"]
        } else {
            planner::TOOLS.iter().map(|t| t.alias).collect()
        };
        aliases.sort_unstable();
        let body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": prompt },
                { "role": "user", "content": query },
            ],
            "stream": false,
            "think": false,
            "options": { "temperature": 0, "num_predict": 80 },
            "keep_alive": "30m",
            "format": look_ai::plan::chat_format(&aliases),
        })
        .to_string();
        let out = ollama::post_json(url, &body, 30).unwrap_or_else(|| "<transport error>".into());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
        println!(
            "--- {label} ({:?}) reason={} eval={} prompt={}\n{}\n",
            domain,
            v["done_reason"],
            v["eval_count"],
            v["prompt_eval_count"],
            v["message"]["content"]
        );
    }
}
