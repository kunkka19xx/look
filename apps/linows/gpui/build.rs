//! Puts the app icon in the Windows exe. Shortcuts and Explorer take theirs
//! from there, and gpui loads the window icon from resource 1.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    {
        const RESOURCES: &str = "lookapp.rc";
        const ICON: &str = "../icons/icon.ico";
        println!("cargo:rerun-if-changed={RESOURCES}");
        println!("cargo:rerun-if-changed={ICON}");
        embed_resource::compile(RESOURCES, embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
