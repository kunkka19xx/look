fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let loaded = look_sources::load_dir(std::path::Path::new(&dir));
    for problem in &loaded.problems {
        println!("problem: {}", problem.message);
    }
    for block in &loaded.blocks {
        println!(
            "block {} name={:?} icon={:?} enabled={} preview={:?} unknown={:?}",
            block.id, block.name, block.icon, block.enabled, block.preview, block.unknown_keys
        );
    }
}
