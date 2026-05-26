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

    tauri_build::build()
}
