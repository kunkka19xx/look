fn main() {
    // Read version from tauri.conf.json so --version matches release tags
    let conf = std::fs::read_to_string("tauri.conf.json").expect("tauri.conf.json");
    let version = conf
        .lines()
        .find(|l| l.contains("\"version\""))
        .and_then(|l| l.split('"').nth(3))
        .expect("version field in tauri.conf.json");
    println!("cargo:rustc-env=APP_VERSION={version}");
    println!("cargo:rerun-if-changed=tauri.conf.json");

    generate_gnome_extension_metadata(version);

    tauri_build::build()
}

/// Generate the GNOME Shell extension `metadata.json` with `version` derived
/// from the app version. GNOME uses this integer to detect that the extension
/// changed and re-install on disk; bumping it on every Look release means the
/// developer never has to remember to update it by hand.
///
/// Mapping: `M.m.p` → `M*10000 + m*100 + p`. Three components, each <100,
/// fit comfortably in a u32 and preserve ordering.
fn generate_gnome_extension_metadata(app_version: &str) {
    let template_path = "src/gnome-shell-extension/metadata.json";
    println!("cargo:rerun-if-changed={template_path}");

    let template = std::fs::read_to_string(template_path).expect("metadata.json template");
    let ext_version = ext_version_from_app(app_version);

    // Substitute the `"version": N` field (or insert if missing) without
    // pulling in a JSON crate just for this. The template is tiny and
    // hand-written, so a small string-level edit is fine.
    let generated = set_json_int_field(&template, "version", ext_version);

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out_path = std::path::Path::new(&out_dir).join("gnome-ext-metadata.json");
    std::fs::write(&out_path, generated).expect("write generated metadata.json");
}

fn ext_version_from_app(v: &str) -> u32 {
    let mut parts = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    major * 10_000 + minor * 100 + patch
}

/// Replace `"key": N` (integer) in `json`. If the key isn't present, insert it
/// before the closing `}`. Whitespace-tolerant for the simple flat object
/// shape we use in metadata.json.
fn set_json_int_field(json: &str, key: &str, value: u32) -> String {
    let needle = format!("\"{key}\"");
    if let Some(start) = json.find(&needle) {
        let after_key = start + needle.len();
        // Skip `: ` (any whitespace) then consume the existing value (digits).
        let rest = &json[after_key..];
        let colon_pos = rest.find(':').expect("malformed JSON: no `:` after key");
        let after_colon = after_key + colon_pos + 1;
        let value_start = after_colon
            + json[after_colon..]
                .chars()
                .take_while(|c| c.is_whitespace())
                .count();
        let value_end = value_start
            + json[value_start..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .count();
        let mut out = String::with_capacity(json.len() + 8);
        out.push_str(&json[..value_start]);
        out.push_str(&value.to_string());
        out.push_str(&json[value_end..]);
        return out;
    }
    // Key missing: insert before the closing brace.
    let brace = json.rfind('}').expect("malformed JSON: no closing brace");
    let prefix = json[..brace].trim_end_matches(|c: char| c.is_whitespace() || c == ',');
    format!("{prefix},\n  \"{key}\": {value}\n}}\n")
}
