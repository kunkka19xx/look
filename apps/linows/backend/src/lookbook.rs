//! The lookbook catalog, and what an entry would do on this machine before
//! installing it. Fetching lives here, as it does in the macOS shell, so the
//! engine makes no network calls; the rest is one hop each into
//! `look_engine::lookbook`.

use std::fs;
use std::path::{Path, PathBuf};

use look_answers::http;
use look_engine::lookbook::{
    InstallOutcome, InstallRequest, NamedFile, Present, PresentRequest, Preview,
};
use serde::Deserialize;

use crate::state::AppState;

const NO_HOME: &str = "no home directory";
const INDEX_URL: &str = "https://raw.githubusercontent.com/kunkka19xx/lookbook/main/index.json";
/// Points the browser at a checkout instead of GitHub, a path or a URL.
/// The same key as macOS.
const INDEX_OVERRIDE: &str = "LOOK_LOOKBOOK_INDEX";
const FETCH_TIMEOUT_SECS: u32 = 15;
const SCHEME_MARK: &str = "://";
const UNREACHABLE: &str = "Could not reach lookbook.";
const UNREADABLE: &str = "Could not read the lookbook catalog.";
/// What the catalog's `platforms` calls this machine.
pub const THIS_PLATFORM: &str = if cfg!(windows) { "windows" } else { "linux" };

/// One ready-made source, as `index.json` describes it.
#[derive(Clone, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Where it works as written. Listed elsewhere, installable only here.
    pub platforms: Vec<String>,
    /// Programs it cannot work without, usually run by a helper script.
    pub requires: Vec<String>,
    /// Declarations, copied into `~/.look/sources`.
    pub files: Vec<String>,
    /// Helper scripts, copied into `~/.look/bin`.
    pub bin: Vec<String>,
    pub readme: String,
    /// Where its files sit, relative to the catalog's base.
    pub path: String,
}

#[derive(Deserialize)]
struct Index {
    base: String,
    /// The same files as rendered pages, for a README somebody clicks.
    web: Option<String>,
    sources: Vec<Entry>,
}

#[derive(Clone)]
pub struct Catalog {
    base: String,
    web: String,
    /// The checkout the override points into, where files are read instead
    /// of fetched.
    local: Option<PathBuf>,
    pub entries: Vec<Entry>,
}

impl Catalog {
    /// Every program any entry asks for, once each.
    pub fn requires(&self) -> Vec<String> {
        let mut wanted: Vec<String> = self
            .entries
            .iter()
            .flat_map(|entry| entry.requires.iter().cloned())
            .collect();
        wanted.sort();
        wanted.dedup();
        wanted
    }

    /// The rendered README on the web, even from a checkout: a local `.md`
    /// opens as raw text in an editor.
    pub fn readme(&self, entry: &Entry) -> Option<String> {
        (!self.web.is_empty()).then(|| format!("{}{}", self.web, entry.readme))
    }

    /// An entry's files in the order they will be written. `Err` rather
    /// than a partial set: a declaration whose script failed to arrive is a
    /// row that does nothing.
    pub fn files(&self, entry: &Entry) -> Result<InstallRequest, String> {
        let read = |name: String, within: &str| {
            let contents = self
                .text(&format!("{}{within}{name}", entry.path))
                .ok_or_else(|| format!("Could not read {} from lookbook.", entry.id))?;
            Ok(NamedFile {
                name,
                contents,
                answers: Vec::new(),
            })
        };
        Ok(InstallRequest {
            declarations: entry
                .files
                .iter()
                .map(|name| read(name.clone(), ""))
                .collect::<Result<_, String>>()?,
            scripts: entry
                .bin
                .iter()
                .map(|name| read(name.clone(), "bin/"))
                .collect::<Result<_, String>>()?,
        })
    }

    fn text(&self, relative: &str) -> Option<String> {
        match &self.local {
            Some(root) => fs::read_to_string(root.join(relative)).ok(),
            None => fetch(&format!("{}{relative}", self.base)),
        }
    }
}

/// Reads the catalog: the override when set, GitHub otherwise.
pub fn catalog() -> Result<Catalog, String> {
    let (body, local) = match std::env::var(INDEX_OVERRIDE).ok().filter(|v| !v.is_empty()) {
        Some(url) if url.contains(SCHEME_MARK) => (fetch(&url), None),
        Some(path) => {
            let path = PathBuf::from(path);
            let root = path.parent().map(Path::to_path_buf);
            (fs::read_to_string(&path).ok(), root)
        }
        None => (fetch(INDEX_URL), None),
    };
    let index: Index =
        serde_json::from_str(&body.ok_or(UNREACHABLE)?).map_err(|_| UNREADABLE.to_string())?;
    Ok(Catalog {
        // Declared by the catalog, never derived from `base`: see "What Look
        // reads from index.json" in lookbook's CONTRIBUTING. Empty only for a
        // catalog old enough to predate the field, and `readme` then offers no
        // link, which is the same as the README being unreachable.
        web: index.web.unwrap_or_default(),
        base: index.base,
        local,
        entries: index.sources,
    })
}

fn fetch(url: &str) -> Option<String> {
    String::from_utf8(http::get_bytes(url, FETCH_TIMEOUT_SECS)?).ok()
}

/// What `toml` declares and what it would need here, or why it is not a file
/// of blocks at all.
pub fn preview(toml: &str) -> Result<Preview, String> {
    look_engine::lookbook::preview(toml, &home()?)
}

/// The same with the answers filled in, so the sheet shows what Install
/// would write.
pub fn preview_filled(file: &NamedFile) -> Result<Preview, String> {
    look_engine::lookbook::preview_filled(file, &home()?)
}

/// Which of a catalog entry's `requires` are not installed here.
pub fn missing_programs(programs: &[String]) -> Vec<String> {
    look_engine::lookbook::missing_programs(programs)
}

/// Which of an entry's files are already on disk, so the sheet can say what
/// it would replace.
pub fn present(request: &PresentRequest) -> Result<Present, String> {
    Ok(look_engine::lookbook::present(request, &home()?))
}

/// Writes an entry and re-reads the sources, which no watcher covers. Queues
/// a full refresh when one is already running.
pub fn install(request: &InstallRequest, state: &AppState) -> Result<InstallOutcome, String> {
    let outcome = look_engine::lookbook::install(request, &home()?);
    if !outcome.written.is_empty() && !state.refresh_sources_now() {
        state.request_index_refresh();
    }
    Ok(outcome)
}

fn home() -> Result<PathBuf, String> {
    crate::sources::home().ok_or_else(|| NO_HOME.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checkout_reads_files_next_to_its_index() {
        let dir = std::env::temp_dir().join(format!("look-lookbook-{}", std::process::id()));
        let entry = dir.join("sources/demo");
        fs::create_dir_all(entry.join("bin")).unwrap();
        fs::write(entry.join("demo.toml"), "[demo]\n").unwrap();
        fs::write(entry.join("bin/demo-rows"), "#!/bin/sh\n").unwrap();
        fs::write(
            dir.join("index.json"),
            r#"{"base":"https://raw.githubusercontent.com/o/r/main/","sources":[{"id":"demo",
            "name":"Demo","description":"","platforms":["linux"],"requires":[],
            "files":["demo.toml"],"bin":["demo-rows"],"readme":"sources/demo/README.md",
            "path":"sources/demo/"}]}"#,
        )
        .unwrap();
        let catalog = Catalog {
            base: String::new(),
            web: "https://github.com/o/r/blob/main/".to_string(),
            local: Some(dir.clone()),
            entries: serde_json::from_str::<Index>(
                &fs::read_to_string(dir.join("index.json")).unwrap(),
            )
            .unwrap()
            .sources,
        };
        let files = catalog.files(&catalog.entries[0]).unwrap();
        assert_eq!(files.declarations[0].contents, "[demo]\n");
        assert_eq!(files.scripts[0].name, "demo-rows");
        assert_eq!(
            catalog.readme(&catalog.entries[0]).as_deref(),
            Some("https://github.com/o/r/blob/main/sources/demo/README.md")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
